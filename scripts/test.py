#!/usr/bin/env python3
"""Run isolated release validation and write a machine-readable command report."""
import argparse
import datetime
import json
import os
import pathlib
import subprocess
import sys
import time

ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser',action='store_true',help='Also run real Chrome UI checks; requires Playwright and Google Chrome')
    parser.add_argument('--report',default='/tmp/plexv2-test-report.json')
    args=parser.parse_args()
    commands=[['cargo','test','--locked'],['cargo','clippy','--all-targets','--locked','--','-D','warnings'],['node','scripts/check-js.cjs'],['node','--check','static/sw.js'],[sys.executable,'scripts/render-help.py','--check'],[sys.executable,'tests/help-renderer.py'],['node','tests/local-grouping.cjs'],['node','tests/movie-collections.cjs'],['node','tests/playback-strategies.cjs'],['node','tests/new-features.cjs'],['node','--test','tests/player-controls.cjs'],['cargo','build','--release','--locked'],[sys.executable,'scripts/integration.py']+(['--browser'] if args.browser else [])]
    env={**os.environ,'PLEXV2_TEST_BINARY':str(ROOT/'target'/'release'/('smart-media-scanner.exe' if os.name=='nt' else 'smart-media-scanner'))}
    results=[]
    try:
        for command in commands:
            print('\nRUN '+ ' '.join(command),flush=True);start=time.monotonic()
            result=subprocess.run(command,cwd=ROOT,env=env)
            results.append({'command':command,'exit_code':result.returncode,'seconds':round(time.monotonic()-start,3)})
            if result.returncode: return result.returncode
        print('\nPASS: all requested validation steps completed',flush=True)
        return 0
    finally:
        report=pathlib.Path(args.report);report.parent.mkdir(parents=True,exist_ok=True)
        report.write_text(json.dumps({'recorded_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'browser_requested':args.browser,'all_passed':len(results)==len(commands) and all(x['exit_code']==0 for x in results),'steps':results},indent=2)+'\n')
        print(f'Report: {report}',flush=True)
if __name__=='__main__':raise SystemExit(main())
