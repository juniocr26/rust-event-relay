import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('demo_token', Path(__file__).with_name('demo-token.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class TokenTests(unittest.TestCase):
    def test_replacement_preserves_other_credentials_permissions_and_inode(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('POSTGRES_PASSWORD=unchanged\nDEMO_TOKEN=' + 'a' * 64 + '\nREPLAY_OPERATORS=unchanged\n')
            path.chmod(0o640)
            before = path.stat()
            module.replace_token(path, 'b' * 64)
            self.assertEqual(module.read_token(path), 'b' * 64)
            self.assertIn('POSTGRES_PASSWORD=unchanged\n', path.read_text())
            self.assertIn('REPLAY_OPERATORS=unchanged\n', path.read_text())
            self.assertEqual(path.stat().st_ino, before.st_ino)
            self.assertEqual(path.stat().st_mode, before.st_mode)

    def test_ambiguous_configuration_is_not_modified(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            for content in ['', 'DEMO_TOKEN=invalid\n', 'DEMO_TOKEN=x\nDEMO_TOKEN=y\n']:
                path.write_text(content)
                with self.assertRaises(ValueError):
                    module.read_token(path)
                if content.count('DEMO_TOKEN=') != 1:
                    with self.assertRaises(ValueError):
                        module.replace_token(path, 'c' * 64)
                    self.assertEqual(path.read_text(), content)

    def test_activation_restarts_only_http_and_checks_both_tokens(self):
        from unittest.mock import patch
        from types import SimpleNamespace
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(returncode=0)) as restart, patch.object(module, 'status', side_effect=[200, 401]) as status, patch('builtins.print') as display:
                module.rotate(path)
                new = module.read_token(path)
                self.assertNotEqual(new, 'a' * 64)
                restart.assert_called_once_with(['supervisorctl', 'restart', 'http'], capture_output=True)
                self.assertEqual(status.call_args_list[0].args, (new,))
                self.assertEqual(status.call_args_list[1].args, ('a' * 64,))
                self.assertIn('Keep this token private', display.call_args_list[0].args[0])
                self.assertEqual(display.call_args_list[1].args, (new,))

    def test_restart_failure_keeps_authoritative_replacement_without_displaying_it(self):
        from unittest.mock import patch
        from types import SimpleNamespace
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(returncode=1)), patch('builtins.print') as display:
                with self.assertRaises(ValueError):
                    module.rotate(path)
                self.assertNotEqual(module.read_token(path), 'a' * 64)
                display.assert_not_called()
