import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch
spec = importlib.util.spec_from_file_location('start_local', Path(__file__).with_name('start-local.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class StartupTests(unittest.TestCase):
    def test_resumes_missing_binary_failures_without_starting_intentional_stops(self):
        with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(stdout='http FATAL missing binary\nrelay RUNNING pid 2\nconsumer STOPPED intentional\n')), patch.object(module, 'run') as run:
            module.resume_failed_processes()
            run.assert_called_once_with('docker', 'compose', 'exec', '-T', 'app', 'supervisorctl', 'start', 'http')

    def test_existing_healthy_manager_is_untouched(self):
        with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(stdout='http RUNNING pid 1\nrelay RUNNING pid 2\nconsumer RUNNING pid 3\n')), patch.object(module, 'run') as run:
            module.resume_failed_processes()
            run.assert_not_called()

    def test_backoff_is_stopped_before_named_restart(self):
        with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(stdout='http BACKOFF retry pending\n')), patch.object(module, 'run') as run:
            module.resume_failed_processes()
            self.assertEqual([call.args[-2:] for call in run.call_args_list], [('stop', 'http'), ('start', 'http')])
