import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch, call

spec=importlib.util.spec_from_file_location('service_check',Path(__file__).with_name('service-check.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

class ServiceTests(unittest.TestCase):
    def test_named_recovery_preserves_operator_stop(self):
        before={'http':'RUNNING','relay':'FATAL','consumer':'STOPPED'}
        with patch.object(module,'states',return_value=before),patch.object(module,'control') as control:
            self.assertEqual(module.activate_http(),before)
            self.assertEqual(control.call_args_list,[call('start','relay'),call('restart','http')])

    def test_stopped_http_is_never_started_implicitly(self):
        with patch.object(module,'states',return_value={'http':'STOPPED','relay':'RUNNING','consumer':'RUNNING'}),patch.object(module,'control') as control:
            with self.assertRaisesRegex(ValueError,'activation pending'):module.activate_http()
            control.assert_not_called()

    def test_backoff_reports_persistent_startup_failure(self):
        with patch.object(module,'states',return_value={'http':'RUNNING','relay':'BACKOFF','consumer':'RUNNING'}),patch.object(module,'control') as control:
            with self.assertRaisesRegex(ValueError,'diagnose startup'):module.activate_http()
            control.assert_not_called()
