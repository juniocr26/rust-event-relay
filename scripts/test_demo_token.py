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

    def test_activation_checks_both_tokens_and_services(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            with patch.object(module.service_check, 'activate_http', return_value={'http':'RUNNING'}) as activate, patch.object(module.service_check, 'verify') as verify, patch.object(module, 'status', side_effect=[200,401]), patch('builtins.print') as display:
                module.rotate(path)
                activate.assert_called_once()
                verify.assert_called_once_with({'http':'RUNNING'})
                self.assertEqual(display.call_args_list[-1].args, (module.read_token(path),))

    def test_service_failure_reports_rotation_already_applied(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            with patch.object(module.service_check, 'activate_http', return_value={'http':'RUNNING'}), patch.object(module.service_check, 'verify', side_effect=ValueError('relay readiness failed')), patch.object(module, 'status', side_effect=[200,401]), patch('builtins.print') as display:
                with self.assertRaisesRegex(ValueError, 'Rotation already applied'):
                    module.rotate(path)
                self.assertNotEqual(module.read_token(path), 'a' * 64)
                display.assert_not_called()

    def test_activation_failure_keeps_authoritative_replacement(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            with patch.object(module.service_check, 'activate_http', side_effect=ValueError('http intentionally STOPPED')), patch.object(module, 'status', return_value=503), patch('builtins.print') as display:
                with self.assertRaisesRegex(ValueError, 'Token saved'):
                    module.rotate(path)
                self.assertNotEqual(module.read_token(path), 'a' * 64)
                display.assert_not_called()

    def test_control_timeout_after_activation_reports_applied_without_low_level_details(self):
        from unittest.mock import patch
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / '.env'
            path.write_text('DEMO_TOKEN=' + 'a' * 64 + '\n')
            failure = subprocess.TimeoutExpired(['sensitive-low-level-argument'], 10)
            with patch.object(module.service_check, 'activate_http', return_value={'http':'RUNNING'}), patch.object(module.service_check, 'verify', side_effect=failure), patch.object(module, 'status', side_effect=[200,401]), patch('builtins.print') as display:
                with self.assertRaisesRegex(ValueError, 'Rotation already applied') as caught:
                    module.rotate(path)
                self.assertNotIn('sensitive-low-level-argument', str(caught.exception))
                display.assert_not_called()
