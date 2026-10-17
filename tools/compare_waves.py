#!/usr/bin/env python3
"""Side-by-side measurements: Jardim Sonoro Pye4060 vs Waves Kramer PIE (Mono).

Usage: python3 compare_waves.py [fr|static|thmap|attack|release|thd|drive|all]

Both plugins get the same stimulus; tables print "ours | waves" (or
ours/waves). Levels are sine-peak dBFS; "GR" is input minus output
fundamental. Waves' VST3 occasionally returns a silent block right after a
parameter change; runs are retried, and a stray ~100+ dB "GR" cell in the
Waves column is that glitch, not a measurement.
"""
import os
import sys

import numpy as np
import pedalboard
W="/Library/Audio/Plug-Ins/VST3/WaveShell1-VST3 17.1.vst3"
SR=48000
def load(name="Kramer PIE Mono"):
    fd=os.dup(2); dn=os.open(os.devnull,os.O_WRONLY); os.dup2(dn,2)
    p=pedalboard.load_plugin(W,plugin_name=name)
    os.dup2(fd,2)
    return p
def setw(p,th=0.0,ratio="3:1 ",decay="4 x100ms",out=0.0,analog="Off "):
    p.threshold=th; p.ratio=ratio; p.decay=decay; p.output=out; p.analog=analog
def run(p,x):
    x=np.asarray(x,np.float32)
    if x.ndim==1: x=x[None,:]
    return p.process(x,SR,reset=True)
def sine(f,db,dur,ph=0):
    t=np.arange(int(dur*SR))/SR
    return (10**(db/20)*np.sin(2*np.pi*f*t+ph)).astype(np.float32)
def harm(y,f,nh=7):
    t=np.arange(len(y))/SR
    cols=[]
    for h in range(1,nh+1):
        cols+= [np.sin(2*np.pi*h*f*t),np.cos(2*np.pi*h*f*t)]
    A=np.stack(cols,1); c,*_=np.linalg.lstsq(A,y,rcond=None)
    return np.array([np.hypot(c[2*i],c[2*i+1]) for i in range(nh)]), np.arctan2(c[1],c[0])
def db(x): return 20*np.log10(np.maximum(np.abs(x),1e-12))

def runr(p,x):
    for _ in range(4):
        y=run(p,x)
        if np.max(np.abs(y))>1e-7 or np.max(np.abs(x))<1e-7: return y
    return y
def fund_db(y,f,n=4800):
    h,_=harm(y[-n:],f,1); return db(h[0])

# --- Pye4060 adapter (same settings vocabulary as Waves)
PYE=os.path.expanduser("~/Library/Audio/Plug-Ins/VST3/Pye4060.vst3")
RMAP={'1:1 ':'1:1','2:1 ':'2:1','3:1 ':'3:1','5:1 ':'5:1','Limit ':'Limit'}
DMAP={'1 x100ms':'100 ms','2 x100ms':'200 ms','4 x100ms':'400 ms','8 x100ms':'800 ms','16 x100ms':'1600 ms','32 x100ms':'3200 ms'}
AMAP={'Off ':'Off','50Hz ':'50 Hz','60Hz ':'60 Hz'}
def loadp():
    p=pedalboard.load_plugin(PYE)
    return p
def setp(p,th=0.0,ratio="3:1 ",decay="4 x100ms",out=0.0,analog="Off ",mode="Stereo"):
    p.processing_mode=mode
    p.threshold_a_db=th; p.ratio_a=RMAP[ratio]; p.decay_a=DMAP[decay]; p.output_a_db=out; p.analog_hum=AMAP[analog]
def run_pye(p,x):
    x=np.asarray(x,np.float32)
    if x.ndim==1: x=np.stack([x,x])
    return p.process(x,SR,reset=True)


if __name__ == "__main__":
    W=load(); P=loadp()
    def runW(x): return runr(W,x)[0]
    def runP(x): return run_pye(P,x)[0]
    def both(cfg):
        setw(W,**cfg); setp(P,**cfg)
    def fund(y,f,n=4800): h,_=harm(y[-n:],f,1); return db(h[0])
    sec=sys.argv[1] if len(sys.argv)>1 else "all"
    if sec in("all","fr"):
        print("== FR @-30 dBFS, 1:1   f: ours / waves")
        both(dict(th=16,ratio='1:1 '))
        out=[]
        for f in [10,20,30,50,100,1000,4000,8000,10000,12000,14000,15000,16000,18000,20000,22000]:
            x=sine(f,-30,0.6); out.append(f"{f}:{fund(runP(x),f,12000)+30:+.2f}/{fund(runW(x),f,12000)+30:+.2f}")
        print(" ".join(out))
    if sec in("all","static"):
        lv=list(range(-30,7,3))
        print("== static GR th=0 @1k (in dBFS peak):"," ".join(f"{l:+5d}" for l in lv))
        for r in ['2:1 ','3:1 ','5:1 ','Limit ']:
            both(dict(th=0,ratio=r,decay='4 x100ms'))
            gp=[l-fund(runP(sine(1000,l,0.8)),1000) for l in lv]
            gw=[l-fund(runW(sine(1000,l,0.8)),1000) for l in lv]
            print(f"{r:6s} ours  "+" ".join(f"{g:5.1f}" for g in gp)); print(f"{'':6s} waves "+" ".join(f"{g:5.1f}" for g in gw))
    if sec in("all","thmap"):
        print("== GR at -10 dBFS vs threshold (3:1): th: ours/waves")
        out=[]
        for th in [-24,-16,-8,0,8,16]:
            both(dict(th=th,ratio='3:1 '))
            x=sine(1000,-10,0.8); out.append(f"{th}:{-10-fund(runP(x),1000):.1f}/{-10-fund(runW(x),1000):.1f}")
        print(" ".join(out))
    if sec in("all","attack"):
        for r in ['3:1 ','Limit ']:
            both(dict(th=0,ratio=r))
            t=np.arange(int(0.6*SR))/SR; amp=np.where(t<0.3,10**(-30/20),10**(-2/20)); x=(amp*np.sin(2*np.pi*1000*t)).astype(np.float32)
            for name,fn in [("ours",runP),("waves",runW)]:
                y=fn(x); n0=int(0.3*SR)
                g=[-db(np.max(np.abs(y[k:k+24]))/np.max(np.abs(x[k:k+24]))) for k in range(n0,n0+24*30,24)]
                print(f"attack {r} {name:5s}"," ".join(f"{v:.1f}" for v in g[:24]))
    if sec in("all","release"):
        for d in ['1 x100ms','4 x100ms','16 x100ms']:
            both(dict(th=0,ratio='3:1 ',decay=d))
            t=np.arange(int(4.0*SR))/SR; amp=np.where(t<1.0,10**(4/20),10**(-30/20)); x=(amp*np.sin(2*np.pi*1000*t)).astype(np.float32)
            for name,fn in [("ours",runP),("waves",runW)]:
                y=fn(x); n0=int(SR)
                pts=[0,10,20,50,100,200,400,800,1600,2900]
                v=[]
                for ms in pts:
                    a=n0+int(ms*SR/1000); v.append(-30-fund(y[a:a+240],1000,240))
                print(f"release {d:9s} {name:5s}"," ".join(f"{ms}ms:{g:.1f}" for ms,g in zip(pts,v)))
    if sec in("all","thd"):
        print("== THD @ +4 dBFS (24 dB over), th 0:  ours | waves")
        for f in [1000,100,50]:
            for r in ['3:1 ','Limit ']:
                for d in ['1 x100ms','4 x100ms','32 x100ms']:
                    both(dict(th=0,ratio=r,decay=d))
                    x=sine(f,4,2.0); res=[]
                    for fn in (runP,runW):
                        y=fn(x); h,_=harm(y[-int(SR*0.2):],f,9)
                        res.append(f"GR{4-db(h[0]):5.1f} THD{np.sqrt(np.sum(h[1:]**2))/h[0]*100:6.3f}% H2{db(h[1]/h[0]):5.0f} H3{db(h[2]/h[0]):5.0f}")
                    print(f"{f:5d} {r:6s} {d:9s} {res[0]} | {res[1]}")
    if sec in("all","drive"):
        print("== 1:1 drive (in-th+20 over turnover): ours | waves")
        for th in [0,-12,-24]:
            both(dict(th=th,ratio='1:1 '))
            row=[]
            for l in [-30,-20,-10,0]:
                x=sine(1000,l,0.6); res=[]
                for fn in (runP,runW):
                    y=fn(x); h,_=harm(y[-9600:],1000,9); res.append(f"{l-db(h[0]):.1f}/{np.sqrt(np.sum(h[1:]**2))/h[0]*100:.2f}%")
                row.append(f"{l:+d}[{l-th+20:+d}] {res[0]} | {res[1]}")
            print(th,"  ".join(row))
