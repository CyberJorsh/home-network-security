"""Generate synthetic packets and validate the installed TShark/collector pipeline."""
import ipaddress
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'debug' / ('hns-collector.exe' if os.name == 'nt' else 'hns-collector')


def packet(src, dst):
    payload = b'synthetic HNS fixture'
    udp = struct.pack('!HHHH', 41000, 42000, 8 + len(payload), 0) + payload
    header = struct.pack('!BBHHHBBH4s4s', 0x45, 0, 20 + len(udp), 1, 0, 64, 17, 0, ipaddress.ip_address(src).packed, ipaddress.ip_address(dst).packed)
    checksum = sum(struct.unpack('!10H', header))
    while checksum >> 16:
        checksum = (checksum & 0xffff) + (checksum >> 16)
    header = header[:10] + struct.pack('!H', (~checksum) & 0xffff) + header[12:]
    ethernet = bytes.fromhex('0200000000020200000000010800')
    return ethernet + header + udp


def main():
    if not shutil.which('tshark'):
        raise SystemExit('TShark is required. This check must not be reported as passed when skipped.')
    subprocess.run(['cargo', 'build', '--locked', '-p', 'hns-collector'], cwd=ROOT, check=True)
    frames = [packet('192.168.1.10', '203.0.113.10'), packet('203.0.113.10', '192.168.1.10'), packet('192.168.1.10', '192.168.1.20'), packet('192.168.1.10', '224.0.0.251')]
    with tempfile.TemporaryDirectory(prefix='hns-pcap-') as directory:
        capture = Path(directory) / 'synthetic.pcap'
        with capture.open('wb') as output:
            output.write(struct.pack('<IHHIIII', 0xa1b2c3d4, 2, 4, 0, 0, 65535, 1))
            for index, frame in enumerate(frames):
                output.write(struct.pack('<IIII', 1780000000 + index, 0, len(frame), len(frame)))
                output.write(frame)
        args = [str(BINARY), '--db', str(Path(directory) / 'test.db'), '--sensor', 'fixture']
        result = subprocess.run(args + ['import', str(capture)], capture_output=True, text=True, check=True)
        assert 'Imported 4 new' in result.stdout, result.stdout
        data = json.loads(subprocess.check_output(args + ['snapshot'], text=True))
        assert data['observationCount'] == 4
        for key in ['upload', 'download', 'localBytes']:
            assert data['totals'][key] == len(frames[0]), (key, data['totals'])
        assert sum(c['bytes'] for c in data['conversations'] if c['direction'] == 'multicast') == len(frames[0])
        result = subprocess.run(args + ['import', str(capture)], capture_output=True, text=True, check=True)
        assert 'Imported 0 new' in result.stdout
        print('PASS: real TShark decoded 4 synthetic frames; WAN, LAN, multicast bytes and reimport idempotency verified')


if __name__ == '__main__':
    main()
