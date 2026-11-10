"""JS-only smoke observations. Obscura does not validate browser layout."""
from pathlib import Path
import json,subprocess
root=Path(__file__).resolve().parent
probe=r'''JSON.stringify((()=>{
 const q=id=>document.getElementById(id);
 const result={scope:'JavaScript only; no layout or real rAF timing assertions',
  initialized:typeof window.mockup,stripCount:document.querySelectorAll('.strip').length,
  qtr570:document.querySelector('.strip[data-cs="QTR570"]').textContent,
  runwayLabels:[...document.querySelectorAll('.runway-label')].map(e=>e.textContent),
  controlsInHeader:[...document.querySelectorAll('.map-controls > button')].map(e=>({id:e.id,inHeader:!!e.closest('.map-heading')})),
  ledgerInitial:q('runway-entries').textContent,
  initialSelected:window.mockup.state.selected};
 select('QTR570');result.selectedAfterSelect=window.mockup.state.selected;
 select('IGO512');q('command').value='IGO512 TAXI 29R VIA A D HOLD SHORT 28';suggest();
 result.sendDisabled=q('send').disabled;
 result.preview=window.mockup.state.preview;
 send();result.phaseAfterSend=window.mockup.state.traffic.find(a=>a.cs==='IGO512').phase;
 result.sentRoute=window.mockup.state.lastRoute;
 result.ledgerAfterSend=q('runway-entries').textContent;
 undo();result.phaseAfterUndo=window.mockup.state.traffic.find(a=>a.cs==='IGO512').phase;
 const spring={position:1,target:1.2,velocity:0,stiffness:400,damping:1};
 result.zeroDtSettled=springStep(spring,0);result.positionAfterZeroDt=spring.position;
 result.nextDtSettled=springStep(spring,.016);result.positionAfter16msIntegration=spring.position;
 return result;
})())'''
observations=[]
for variant in 'abc':
 run=subprocess.run(['obscura','fetch',(root/(variant+'.html')).as_uri(),'--wait','0','--eval',probe],capture_output=True,text=True,timeout=60)
 row={'direction':variant,'exit_code':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
 observations.append(row)
 print(variant,run.returncode,run.stdout.strip(),run.stderr.strip(),flush=True)
(root/'obscura-smoke.json').write_text(json.dumps(observations,indent=2))
