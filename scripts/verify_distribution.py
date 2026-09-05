"""Verify an already-built release artifact. Never installs, signs, or publishes."""
import argparse
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path, help='A signed .app on macOS or signed .exe on Windows')
    args = parser.parse_args()
    artifact = args.artifact.resolve(strict=True)
    if sys.platform == 'darwin' and artifact.suffix == '.app':
        result = subprocess.run(['codesign', '-dv', '--verbose=4', str(artifact)], capture_output=True, text=True, check=True)
        if 'Authority=Developer ID Application:' not in result.stderr:
            raise SystemExit('Not a Developer ID Application signature. Development/ad-hoc builds are not distribution-ready.')
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(artifact)], check=True)
        subprocess.run(['xcrun', 'stapler', 'validate', str(artifact)], check=True)
        subprocess.run(['spctl', '--assess', '--type', 'execute', str(artifact)], check=True)
    elif sys.platform == 'win32' and artifact.suffix == '.exe':
        # Pass the path as an argument, never interpolate user text into PowerShell code.
        import os
        env = dict(os.environ, HNS_VERIFY_ARTIFACT=str(artifact))
        subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', "$s=Get-AuthenticodeSignature -LiteralPath $env:HNS_VERIFY_ARTIFACT; if ($s.Status -ne 'Valid') { Write-Error 'Installer signature is not valid'; exit 1 }; if (-not $s.TimeStamperCertificate) { Write-Error 'Installer is missing a trusted timestamp'; exit 1 }"], env=env, check=True)
    else:
        raise SystemExit('Run on the matching platform with a .app or .exe artifact.')
    print('PASS: platform signature checks. Fresh-machine installation and capture acceptance are still required.')


if __name__ == '__main__':
    main()
