#!/usr/bin/env python3
"""Isolated native UDP qualification. Run with sudo; touches only owned namespaces.

Two endpoints and a forwarding router; endpoint netem adds propagation/loss,
router HTB+bfifo supplies the independent bounded bottleneck queue. No host
interface/qdisc/sysctl/firewall changes. All namespaces/processes removed finally.
"""
import argparse
import json
import os
import pathlib
import signal
import subprocess
import time


def run(*args, **kwargs):
    return subprocess.run([str(x) for x in args], check=True, **kwargs)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--binary', required=True)
    p.add_argument('--output', required=True)
    p.add_argument('--trials', type=int, default=3)
    p.add_argument('--seconds', type=int, default=12)
    p.add_argument('--warmup', type=int, default=2)
    p.add_argument('--unseeded-netem', action='store_true', help='For older iproute2 without netem seed support')
    p.add_argument('--only', help='Comma-separated case names; supplementary cases require explicit selection')
    opt = p.parse_args()
    out = pathlib.Path(opt.output).resolve()
    out.mkdir(parents=True, exist_ok=True)
    binary = pathlib.Path(opt.binary).resolve()
    prefix = f'nqb-{os.getpid()}'
    c, r, s = [prefix + x for x in ('c', 'r', 's')]
    owned = []
    proc = None
    changes = []

    def ns(n, *cmd, **kw):
        return run('ip', 'netns', 'exec', n, *cmd, **kw)

    def command(n, *cmd):
        return ['ip', 'netns', 'exec', n, *map(str, cmd)]

    def bandwidth(rate, queue_ms):
        for dev in ('rc', 'rs'):
            ns(r, 'tc', 'class', 'replace', 'dev', dev, 'parent', '1:', 'classid', '1:1',
               'htb', 'rate', f'{rate}mbit', 'ceil', f'{rate}mbit', 'burst', '3000', 'cburst', '3000', 'quantum', '1514')
            ns(r, 'tc', 'qdisc', 'replace', 'dev', dev, 'parent', '1:1', 'handle', '10:',
               'bfifo', 'limit', str(max(3000, int(rate * 125 * queue_ms))))

    def stop_server():
        nonlocal proc
        if proc and proc.poll() is None:
            os.killpg(proc.pid, signal.SIGTERM)
            try: proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
        proc = None

    def terminate(signum, frame):
        raise KeyboardInterrupt(f'signal {signum}')

    signal.signal(signal.SIGTERM, terminate)
    signal.signal(signal.SIGINT, terminate)
    try:
        for name in (c, r, s):
            run('ip', 'netns', 'add', name)
            owned.append(name)
            ns(name, 'ip', 'link', 'set', 'lo', 'up')
        for endpoint, eif, rif, eaddr, raddr in (
            (c, 'ce', 'rc', '198.18.0.2/30', '198.18.0.1/30'),
            (s, 'se', 'rs', '198.18.0.6/30', '198.18.0.5/30'),
        ):
            ns(endpoint, 'ip', 'link', 'add', eif, 'type', 'veth', 'peer', 'name', rif)
            ns(endpoint, 'ip', 'link', 'set', rif, 'netns', r)
            ns(endpoint, 'ip', 'addr', 'add', eaddr, 'dev', eif)
            ns(r, 'ip', 'addr', 'add', raddr, 'dev', rif)
            ns(endpoint, 'ip', 'link', 'set', eif, 'up')
            ns(r, 'ip', 'link', 'set', rif, 'up')
            ns(endpoint, 'ip', 'route', 'add', 'default', 'via', raddr.split('/')[0])
            ns(r, 'tc', 'qdisc', 'add', 'dev', rif, 'root', 'handle', '1:', 'htb', 'default', '1')
        ns(r, 'sysctl', '-qw', 'net.ipv4.ip_forward=1')
        metadata = {'hostname': os.uname().nodename, 'uname': list(os.uname()),
                    'binary_sha256': subprocess.check_output(['sha256sum', str(binary)], text=True).split()[0],
                    'started_unix': time.time(), 'options': vars(opt)}
        (out/'metadata.json').write_text(json.dumps(metadata, indent=2))
        with (out/'offloads.txt').open('w') as f:
            ns(s, 'ethtool', '-k', 'se', stdout=f, stderr=subprocess.STDOUT)
        cases = [
            ('clean', 20, 20, 100, 0, False, False),
            ('wan', 20, 80, 100, 0, False, False),
            ('loss-1pct', 20, 80, 100, 1, False, False),
            ('deep-queue', 20, 40, 500, 0, False, False),
            ('shallow-queue', 20, 80, 10, 0, False, False),
            ('recovery', 20, 80, 100, 0.1, False, False),
            ('rpc-only', 20, 80, 100, 0.1, False, True),
            ('wan-no-gso', 20, 80, 100, 0, True, False),
        ]
        if opt.only:
            cases += [('ecn-aqm',20,80,100,0,False,False),
                      ('jitter-reorder',20,80,100,0,False,False),
                      ('high-bdp',100,80,100,0,False,False)]
            cases=[case for case in cases if case[0] in opt.only.split(',')]
            if not cases: raise ValueError('no cases selected')
        for name, rate, rtt, queue, loss, no_gso, rpc_only in cases:
            for trial in range(opt.trials):
                order = ('cubic','bbr3') if trial % 2 == 0 else ('bbr3','cubic')
                for cc in order:
                    for dev in ('rc','rs'):
                        subprocess.run(command(r,'tc','qdisc','del','dev',dev,'parent','1:1'),
                                       stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                    bandwidth(rate, queue)
                    if name == 'ecn-aqm':
                        for dev in ('rc','rs'):
                            ns(r,'tc','qdisc','del','dev',dev,'parent','1:1')
                            ns(r,'tc','qdisc','add','dev',dev,'parent','1:1','handle','10:',
                               'fq_codel','limit','10000','target','5ms','interval','100ms','ecn')
                    for endpoint, dev in ((c,'ce'),(s,'se')):
                        ns(endpoint, 'tc', 'qdisc', 'replace', 'dev', dev, 'root',
                           'netem', 'limit', '100000', 'delay', f'{rtt/2}ms',
                           *(['5ms','distribution','normal'] if name=='jitter-reorder' else []),
                           'loss', 'random', f'{loss}%',
                           *(['reorder','1%','50%'] if name=='jitter-reorder' else []),
                           *([] if opt.unseeded_netem else ['seed',str(12345+trial)]))
                    tag = f'{name}-{trial}-{cc}'
                    cert = out/'cert.der'
                    cert.unlink(missing_ok=True)
                    flags = (['--no-gso'] if no_gso else []) + ['--fixed-mtu']
                    seconds = 24 if name == 'recovery' else opt.seconds
                    serverlog = (out/f'{tag}.server.log').open('w')
                    proc = subprocess.Popen(command(s, '/usr/bin/time', '-v', binary,
                        '--server', '--address', '198.18.0.6:44330', '--controller', cc,
                        '--cert', cert, *flags), stdout=serverlog, stderr=subprocess.STDOUT,
                        start_new_session=True)
                    try:
                        for _ in range(100):
                            if cert.exists() and cert.stat().st_size: break
                            if proc.poll() is not None: raise RuntimeError('server exited')
                            time.sleep(.05)
                        else: raise RuntimeError('certificate not ready')
                        time.sleep(.1)
                        err = (out/f'{tag}.client.log').open('w')
                        client = subprocess.Popen(command(c, '/usr/bin/time', '-v', binary,
                            '--address', '198.18.0.6:44330', '--controller', cc,
                            '--cert', cert, '--seconds', seconds, '--warmup', opt.warmup,
                            '--label', name, '--trial', trial, *flags,
                            *(['--rpc-only'] if rpc_only else [])),
                            stdout=subprocess.PIPE, stderr=err, text=True, start_new_session=True)
                        start = time.monotonic()
                        if name == 'recovery':
                            for age, newrate in ((8,5),(16,20)):
                                time.sleep(max(0, start+age-time.monotonic()))
                                bandwidth(newrate, queue)
                                changes.append({'tag':tag,'age':time.monotonic()-start,'mbps':newrate})
                        try: stdout, _ = client.communicate(timeout=seconds+opt.warmup+20)
                        except BaseException:
                            os.killpg(client.pid, signal.SIGKILL); client.wait(); raise
                        err.close()
                        if client.returncode:
                            failure={'label':name,'trial':trial,'controller':cc,
                                     'host':os.uname().nodename,'client_exit':client.returncode}
                            with (out/'failed-connections.jsonl').open('a') as f:
                                f.write(json.dumps(failure)+'\n')
                            print(f'{tag}: CONNECTION FAILED; see client log',flush=True)
                            continue
                        row=json.loads(stdout)
                        row['host']=os.uname().nodename
                        row['network']={'rate_mbps':rate,'rtt_ms':rtt,'queue_ms':queue,'loss_pct':loss}
                        with (out/'native.jsonl').open('a') as f: f.write(json.dumps(row)+'\n')
                        print(f"{tag}: {row['bulk_mbps']:.2f} Mbps p95={row['p95_ms']:.1f}ms failed={row['failed_probes']}",flush=True)
                        with (out/f'{tag}.qdisc.txt').open('w') as f:
                            for dev in ('rc','rs'): ns(r,'tc','-s','qdisc','show','dev',dev,stdout=f)
                    finally:
                        stop_server(); serverlog.close()
        (out/'capacity-changes.json').write_text(json.dumps(changes,indent=2))
    finally:
        stop_server()
        for name in reversed(owned):
            # Kill only processes in this run's namespaces, then delete them.
            pids=subprocess.run(['ip','netns','pids',name],capture_output=True,text=True).stdout.split()
            for pid in pids:
                try: os.kill(int(pid),signal.SIGKILL)
                except ProcessLookupError: pass
            subprocess.run(['ip','netns','del',name],check=False)
        (out/'cleanup.json').write_text(json.dumps({'owned_namespaces':owned,'remaining':
            subprocess.check_output(['ip','netns','list'],text=True)}))


if __name__ == '__main__': main()
