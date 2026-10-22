#!/usr/bin/env python3
"""Compare physical LAN and Internet paths. Run locally after native_matrix.py.

Uses only the task's scratch dirs. The Hetzner ingress exception is limited to
OVH's source IP and is deleted by its exact nft handle in finally. All servers
have a timeout and an owned process group, killed after each trial.
"""
import argparse
import json
import os
import pathlib
import shlex
import subprocess
import time

BASE='/tmp/noq-bbr-eval-20261022'
HOSTS={'lan':'emrul@192.168.1.140','ovh':'ubuntu@51.195.190.65','hetzner':'ubuntu@138.201.60.36'}
IPS={'lan':'192.168.1.140','ovh':'51.195.190.65','hetzner':'138.201.60.36'}


def remote(host, args, **kw):
    return subprocess.run(['ssh','-o','BatchMode=yes',HOSTS[host],shlex.join(map(str,args))],check=True,**kw)


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--local-binary',required=True)
    p.add_argument('--output',required=True)
    p.add_argument('--trials',type=int,default=3)
    p.add_argument('--seconds',type=int,default=10)
    p.add_argument('--default-flow-control',action='store_true')
    p.add_argument('--only',help='Comma-separated path names')
    p.add_argument('--remote-binary',default=BASE+'/native-v2')
    opt=p.parse_args()
    out=pathlib.Path(opt.output).resolve(); out.mkdir(parents=True,exist_ok=True)
    handle=None
    group=None
    server=None
    comment=f'noq-bbr-eval-20261022-{os.getpid()}'
    cases=[('physical-lan','lan',None,False),
           ('ovh-to-lan','ovh','lan',False),
           ('hetzner-to-ovh','hetzner','ovh',False),
           ('ovh-to-hetzner','ovh','hetzner',False),
           ('hetzner-to-ovh-no-gso','hetzner','ovh',True)]
    if opt.only: cases=[case for case in cases if case[0] in opt.only.split(',')]
    if not cases: raise ValueError('no paths selected')
    needs_firewall=any(case[1]=='hetzner' for case in cases)
    try:
        # No rule to a host-wide interface/qdisc is needed on OVH or the LAN.
        if needs_firewall:
            remote('hetzner',['sudo','nft','insert','rule','inet','kasm_edge','ingress',
                'ip','saddr',IPS['ovh'],'udp','dport','44330','counter','accept',
                'comment',json.dumps(comment)])
            rules=json.loads(remote('hetzner',['sudo','nft','-j','-a','list','chain','inet','kasm_edge','ingress'],capture_output=True,text=True).stdout)
            matches=[x['rule']['handle'] for x in rules['nftables'] if x.get('rule',{}).get('comment')==comment]
            if len(matches)!=1: raise RuntimeError(f'expected one owned firewall rule, found {matches}')
            handle=matches[0]
        for label,server,client,no_gso in cases:
            if opt.default_flow_control: label+='-default-flow'
            for trial in range(opt.trials):
                for cc in (('cubic','bbr3') if trial%2==0 else ('bbr3','cubic')):
                    tag=f'{label}-{trial}-{cc}'
                    flags=(['--no-gso'] if no_gso else [])+(['--default-flow-control'] if opt.default_flow_control else [])
                    cert=f'{BASE}/paths-cert.der'
                    remote(server,['rm','-f',cert])
                    args=['setsid','timeout','90s','/usr/bin/time','-v',opt.remote_binary,
                          '--server','--once','--address',f'{IPS[server]}:44330','--controller',cc,
                          '--cert',cert,*flags]
                    command=shlex.join(args)+f' > {BASE}/{tag}.server.log 2>&1 < /dev/null & echo $!'
                    result=subprocess.run(['ssh',HOSTS[server],command],check=True,capture_output=True,text=True)
                    group=int(result.stdout.strip())
                    try:
                        for _ in range(50):
                            result=subprocess.run(['ssh',HOSTS[server],f'test -s {cert}'],capture_output=True)
                            if result.returncode==0: break
                            time.sleep(.1)
                        else: raise RuntimeError('server cert not ready')
                        localcert=out/'cert.der'
                        subprocess.run(['scp',HOSTS[server]+':'+cert,str(localcert)],check=True)
                        if client is not None:
                            subprocess.run(['scp',str(localcert),HOSTS[client]+':'+BASE+'/client-cert.der'],check=True)
                        args=[opt.local_binary if client is None else opt.remote_binary,
                              '--address',f'{IPS[server]}:44330','--controller',cc,
                              '--cert',str(localcert) if client is None else BASE+'/client-cert.der',
                              '--seconds',str(opt.seconds),'--warmup','2','--label',label,'--trial',str(trial),*flags]
                        with (out/f'{tag}.client.log').open('w') as err:
                            if client is None:
                                result=subprocess.run(args,check=True,stdout=subprocess.PIPE,stderr=err,text=True,timeout=opt.seconds+30)
                            else:
                                result=remote(client,['/usr/bin/time','-v',*args],stdout=subprocess.PIPE,stderr=err,text=True,timeout=opt.seconds+30)
                        row=json.loads(result.stdout)
                        row.update(server=server,client=client or 'mac',server_ip=IPS[server])
                        with (out/'native.jsonl').open('a') as f: f.write(json.dumps(row)+'\n')
                        print(f"{tag}: {row['bulk_mbps']:.2f} Mbps p95={row['p95_ms']:.1f}ms failed={row['failed_probes']}",flush=True)
                    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
                        failure={'label':label,'trial':trial,'controller':cc,'server':server,
                                 'client':client or 'mac','error':str(exc),
                                 'client_log':str(out/f'{tag}.client.log')}
                        with (out/'failed-connections.jsonl').open('a') as f:
                            f.write(json.dumps(failure)+'\n')
                        print(f'{tag}: CONNECTION FAILED; see client log',flush=True)
                    finally:
                        if group:
                            time.sleep(.3)  # Allow the one-connection server/time wrapper to exit.
                            subprocess.run(['ssh',HOSTS[server],f'kill -TERM -- -{group} 2>/dev/null || true'],check=True)
                            group=None
                        subprocess.run(['scp',HOSTS[server]+':'+BASE+f'/{tag}.server.log',str(out/f'{tag}.server.log')],check=True)
    finally:
        if group and server:
            subprocess.run(['ssh',HOSTS[server],f'kill -TERM -- -{group} 2>/dev/null || true'])
        if handle is not None:
            remote('hetzner',['sudo','nft','delete','rule','inet','kasm_edge','ingress','handle',handle])
        # Also verify by comment, including failure before handle capture.
        rules=json.loads(remote('hetzner',['sudo','nft','-j','-a','list','chain','inet','kasm_edge','ingress'],capture_output=True,text=True).stdout) if needs_firewall else {'nftables':[]}
        leftovers=[x['rule']['handle'] for x in rules['nftables'] if x.get('rule',{}).get('comment')==comment]
        for h in leftovers:
            remote('hetzner',['sudo','nft','delete','rule','inet','kasm_edge','ingress','handle',h])
        (out/'cleanup.json').write_text(json.dumps({'firewall_rule_handle_removed':handle,'extra_handles_removed':leftovers}))


if __name__=='__main__': main()
