// Web equivalent of the pure Rust contracts. No native Windows access.
export class Transition {
  expanded = false; from = 0; start = 0; duration = 0; value = 0;
  sample(now: number) {
    const t = this.duration ? Math.max(0, Math.min(1, (now-this.start)/this.duration)) : 1;
    this.value = this.from + ((this.expanded ? 1 : 0)-this.from)*t*t*(3-2*t);
    return this.value;
  }
  set(expanded: boolean, now: number, animate = true) {
    this.sample(now);
    if (animate && expanded === this.expanded) return;
    this.from=this.value; this.start=now; this.expanded=expanded;
    this.duration=(expanded ? 220 : 180)*Math.abs(Number(expanded)-this.from);
    if (!animate) { this.duration=0; this.from=Number(expanded); this.value=this.from; }
  }
  active() { return Math.abs(this.value-Number(this.expanded))>0.00001; }
}
export function visible(manual: boolean, fullscreen: boolean, concentration: boolean) {
  return manual || fullscreen ? 'hidden' : concentration ? 'concentration' : 'normal';
}
export function boundedPosition(x: number,y: number,w: number,h: number,iw=384,ih=168) {
  return {x:Math.max(iw/2,Math.min(w-iw/2,x)),y:Math.max(8,Math.min(h-ih-52,y))};
}
export type TimerMode = 'timer'|'pomodoro';
export type Phase = 'countdown'|'work'|'short'|'long';
export class Countdown {
  mode: TimerMode='timer'; phase: Phase='countdown'; state: 'ready'|'running'|'paused'|'finished'='ready';
  completed=0; deadline=0; paused=0;
  config={minutes:25,work:25,short:5,long:15};
  duration() { return this.config[this.phase==='countdown'?'minutes':this.phase]*60000; }
  remaining(now:number) { return this.state==='running'?Math.max(0,this.deadline-now):this.state==='paused'?this.paused:this.state==='finished'?0:this.duration(); }
  settle(now:number) { if(this.state==='running' && now>=this.deadline) {this.state='finished';if(this.phase==='work')this.completed++;return true;}return false; }
  primary(now:number) {
    if(this.settle(now))return; // A late Pause never starts a new phase.
    if(this.state==='running'){this.paused=this.remaining(now);this.state='paused';return;}
    if(this.state==='paused'){this.deadline=now+this.paused;this.state='running';return;}
    if(this.state==='finished'){
      if(this.phase==='work')this.phase=this.completed===4?'long':'short';
      else if(this.phase!=='countdown'){if(this.phase==='long')this.completed=0;this.phase='work';}
    }
    this.deadline=now+this.duration();this.state='running';
  }
  reset() {this.state='ready';this.completed=0;this.phase=this.mode==='timer'?'countdown':'work';}
  configure(mode:TimerMode, config:typeof this.config) {
    if(!['ready','finished'].includes(this.state)||!Object.values(config).every(v=>Number.isInteger(v)&&v>=1&&v<=240))return false;
    this.mode=mode;this.config={...config};this.reset();return true;
  }
  pause(now:number) {this.settle(now);if(this.state==='running')this.primary(now);}
  label(){return this.state==='running'?'Pause':this.state==='paused'?'Reprendre':this.state==='finished'?(this.phase==='work'?(this.completed===4?'Pause longue':'Pause courte'):this.phase==='countdown'?'Recommencer':'Travail suivant'):'Démarrer';}
}
export function clock(ms:number) {const s=Math.ceil(Math.max(0,ms)/1000);return `${String(Math.floor(s/60)).padStart(2,'0')}:${String(s%60).padStart(2,'0')}`;}
