"""Run from research/work; every artifact stays under out/ui-design."""
from pathlib import Path
from playwright.sync_api import sync_playwright, TimeoutError as PlaywrightTimeoutError
import json, math, re, hashlib
ROOT=Path(__file__).resolve().parent
rows=[]

def poll(page,key,label,condition,observed,timeout=1000,arg=None):
 """Bounded state polling; report the actual state when the ceiling expires."""
 try:
  handle=page.wait_for_function(condition,arg=arg,polling=10,timeout=timeout)
 except PlaywrightTimeoutError as error:
  value=page.evaluate(observed,arg)
  raise AssertionError(f'{key}: {label} timed out after {timeout} ms; observed {value!r}') from error
 value=handle.json_value();handle.dispose()
 return value

def expect(key,label,actual,expected):
 assert actual==expected,f'{key}: {label}: expected {expected!r}, observed {actual!r}'

CAMERA='() => ({...mockup.state.camera})'
CAMERA_HOME='() => {const c=mockup.state.camera;return c.target===1 && c.position===1 && c.velocity===0}'
DIALOG='''() => {const d=document.querySelector('#result');return {
 open:d.open,modal:d.matches(':modal'),shown:d.classList.contains('shown'),
 opacity:getComputedStyle(d).opacity,transition:getComputedStyle(d).transitionDuration,
 reduced:matchMedia('(prefers-reduced-motion: reduce)').matches,focus:document.activeElement.id}}'''
DIALOG_CLOSED='''() => {const d=document.querySelector('#result');return !d.open &&
 !d.matches(':modal') && document.activeElement.id==='results'}'''

def channels(c):
 if c.startswith('#'):
  c=c[1:]
  if len(c)==3:c=''.join(x*2 for x in c)
  return [int(c[i:i+2],16) for i in (0,2,4)]
 return list(map(float,re.findall(r'[\d.]+',c)))[:3]
def lum(c):
 vals=[v/255 for v in c]
 return sum(w*(v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4) for w,v in zip([.2126,.7152,.0722],vals))
def contrast(a,b):
 a,b=lum(a),lum(b)
 return (max(a,b)+.05)/(min(a,b)+.05)

with sync_playwright() as p:
 browser=p.chromium.launch(executable_path='/usr/bin/chromium',headless=True,args=['--no-sandbox'])
 version=browser.version
 for v in 'abc':
  for width,height in [(1500,844),(1920,1080)]:
   for theme in ['light','dark']:
    for motion in ['no-preference','reduce']:
     key=f'{v}-{width}-{theme}-'+('reduced' if motion=='reduce' else 'motion')
     context=browser.new_context(viewport={'width':width,'height':height},color_scheme=theme,reduced_motion=motion,device_scale_factor=1)
     page=context.new_page();errors=[];network=[]
     page.on('console',lambda m: errors.append('console: '+m.text) if m.type=='error' else None)
     page.on('pageerror',lambda e:errors.append('page: '+str(e)))
     page.on('request',lambda r:network.append(r.url) if not r.url.startswith('file:') else None)
     page.goto((ROOT/(v+'.html')).as_uri())
     poll(page,key,'initial render',
      '() => !!window.mockup && document.querySelector("#map").width > 1 && document.querySelector("#map").height > 1',
      '() => ({mockup:typeof window.mockup,width:document.querySelector("#map").width,height:document.querySelector("#map").height})')
     expect(key,'initial theme',page.locator('html').get_attribute('data-theme'),theme)
     expect(key,'initial strip count',page.locator('.strip').count(),12)
     expect(key,'initial visible aircraft count',page.locator('.plane:visible').count(),12)
     expect(key,'initial metric count',page.locator('.metrics > .metric').count(),3)
     expect(key,'initial suggestions',page.locator('.suggestion').all_text_contents(),['29R','27','STAND'])
     layout=page.evaluate('''()=>{
      const rect=e=>e.getBoundingClientRect();
      const controls=[...document.querySelectorAll('button,input,a')].filter(e=>{const r=rect(e);return r.width&&r.height&&getComputedStyle(e).visibility!=='hidden'});
      const small=controls.filter(e=>rect(e).width<43.99||rect(e).height<43.99).map(e=>e.id||e.textContent);
      const clip=[];for(const s of document.querySelectorAll('.strip')){const r=rect(s);let n=s.parentElement;while(n&&n!==document.body){if(['auto','hidden','scroll'].includes(getComputedStyle(n).overflowY)){const a=rect(n);if(r.bottom>a.bottom+1||r.top<a.top-1){clip.push(s.dataset.cs);break}}n=n.parentElement}}
      const app=rect(document.querySelector('.app')),main=rect(document.querySelector('.play'));
      const planes=[...document.querySelectorAll('.plane')];const overlap=[];
      for(let i=0;i<planes.length;i++)for(let j=i+1;j<planes.length;j++){const a=rect(planes[i]),b=rect(planes[j]);if(a.left<b.right&&a.right>b.left&&a.top<b.bottom&&a.bottom>b.top)overlap.push([planes[i].id,planes[j].id])}
      const mapControls=[...document.querySelectorAll('.map-controls > *')];
      const runwayLabels=[...document.querySelectorAll('.runway-label')].filter(e=>!e.hidden && rect(e).width && rect(e).height);
      const runwayOverlap=[];
      for(const control of mapControls)for(const label of runwayLabels){const a=rect(control),b=rect(label);if(a.left<b.right&&a.right>b.left&&a.top<b.bottom&&a.bottom>b.top)runwayOverlap.push({control:control.id,label:label.textContent,controlBox:a.toJSON(),labelBox:b.toJSON()})}
      return {small,clip,overlap,runwayOverlap,mapControlCount:mapControls.length,runwayLabelCount:runwayLabels.length,overflowX:document.documentElement.scrollWidth-innerWidth,overflowY:document.body.scrollHeight-innerHeight,used:main.width/innerWidth,left:main.left,right:innerWidth-main.right,minTarget:Math.min(...controls.map(e=>Math.min(rect(e).width,rect(e).height))),spring:getComputedStyle(document.querySelector('.strip')).transitionTimingFunction,transition:getComputedStyle(document.querySelector('.strip')).transitionDuration,corner:CSS.supports('corner-shape','squircle')};
     }''')
     assert not layout['small'],(key,'small',layout)
     assert not layout['clip'],(key,'clipped',layout)
     assert not layout['overlap'],(key,'plane targets overlap',layout)
     expect(key,'map controls measured',layout['mapControlCount'],3)
     expect(key,'visible runway end labels measured',layout['runwayLabelCount'],8)
     assert not layout['runwayOverlap'],(key,'map controls overlap runway labels',layout['runwayOverlap'])
     assert layout['overflowX']==0 and layout['overflowY']==0,(key,'overflow',layout)
     assert layout['used']>=.65 and abs(layout['left']-layout['right'])<1,(key,layout)
     assert (layout['transition']=='0s')==(motion=='reduce'),(key,layout)
     assert 'linear(' in layout['spring'] or motion=='reduce'
     tokens=page.evaluate("()=>Object.fromEntries(['bg','surface','soft','ink','muted','line','control','accent','tint','red','redsoft','map','apron','taxi','dialog','dialogink','dialogmuted'].map(n=>[n,getComputedStyle(document.documentElement).getPropertyValue('--'+n).trim()]))")
     ratios={}
     for fg,bg in [('ink','surface'),('muted','surface'),('muted','soft'),('muted','map'),('ink','tint'),('red','redsoft'),('dialogink','dialog'),('dialogmuted','dialog')]:
      r=contrast(channels(tokens[fg]),channels(tokens[bg]));ratios[fg+'/'+bg]=round(r,3);assert r>=4.5,(key,fg,bg,r)
     for fg,bg in [('control','surface'),('taxi','map'),('taxi','apron')]:
      r=contrast(channels(tokens[fg]),channels(tokens[bg]));ratios[fg+'/'+bg]=round(r,3);assert r>=3,(key,fg,bg,r)
     page.screenshot(path=str(ROOT/'shots'/(key+'.png')))
     # Input grammar, keyboard selection and exact preview identity.
     page.locator('#command').fill('IGO512 TAKEOFF 29R');assert page.locator('#send').is_disabled()
     assert page.locator('#command').get_attribute('aria-invalid')=='true'
     page.locator('#command').fill('SEJ204 ');assert page.locator('.suggestion').all_text_contents()==['PUSH']
     page.locator('#command').fill('IGO512 TAXI ');page.locator('#command').press('ArrowDown');page.locator('#command').press('Tab');assert page.locator('#command').input_value()=='IGO512 TAXI 27 '
     page.locator('#command').fill('IGO512 TAXI 29R VIA A D HOLD SHORT 28')
     route=page.evaluate('mockup.state.preview');assert route and route['edges']==['A','D','H28','29R']
     page.locator('#send').click();assert page.evaluate('mockup.state.lastRoute')==route
     assert page.evaluate('mockup.state.traffic.find(a=>a.cs==="IGO512").phase')=='Taxi'
     toast_box=page.locator('#toast').bounding_box();assert abs(toast_box['x']+toast_box['width']/2-width/2)<1,(key,'toast not centred',toast_box)
     page.locator('#toast').hover()
     hover=page.evaluate('() => ({left:mockup.state.undoLeft,start:performance.now()})')
     # Observe the whole interval: an immediate successful poll would not test a pause.
     held=poll(page,key,'undo timer stays paused during 250 ms hover',
      '''({left,start}) => {const value=mockup.state.undoLeft,elapsed=performance.now()-start;
       return (Math.abs(value-left)>=110 || elapsed>=250) ? {left,value,elapsed} : false}''',
      '({left,start}) => ({left,value:mockup.state.undoLeft,elapsed:performance.now()-start})',arg=hover)
     assert abs(held['value']-held['left'])<110,(key,'hover changed undo timer',held)
     page.locator('#undo').click();assert page.evaluate('mockup.state.traffic.find(a=>a.cs==="IGO512").phase')=='Holding'
     # Real map destination click, alternate route, shared legal grammar.
     page.locator('#destination').click();assert page.locator('#send').is_enabled()
     assert page.evaluate('mockup.state.preview.edges')==['A','D','H28','29R']
     page.locator('#command').fill('IGO512 TAXI 29R VIA A B HOLD SHORT 28');assert page.evaluate('mockup.state.preview.edges')==['A','B','H28','D','29R']
     # Persistent alert, keyboard hold, undo restores conflict.
     page.locator('#focus-conflict').click();assert page.evaluate('mockup.state.selected')=='AIC304'
     page.locator('#map').press('h');page.locator('#command').press('Enter');assert page.evaluate('mockup.state.activeConflict') is False
     assert page.locator('#conflict-title').inner_text()=='PAIR HELD / TAXIWAY D'
     page.locator('#map').focus();page.keyboard.press('Control+z');assert page.evaluate('mockup.state.activeConflict') is True
     # Aircraft + strip paths and native focus navigation.
     page.locator('#plane-SEJ204').click();expect(key,'aircraft click selection',page.locator('#selected-call').inner_text(),'SEJ204');expect(key,'aircraft click focus',page.evaluate('document.activeElement.id'),'map')
     page.locator('.strip[data-cs="IGO512"]').click();expect(key,'strip click selection',page.evaluate('mockup.state.selected'),'IGO512')
     page.locator('#map').focus();page.keyboard.press('Tab');expect(key,'Tab aircraft selection',page.evaluate('mockup.state.selected'),'SEJ204')
     page.locator('#command').focus();page.locator('#command').press('Escape');expect(key,'command Escape focus',page.evaluate('document.activeElement.id'),'map')
     # Interrupt zoom by reversing the target before it settles.
     page.locator('#zoom-in').click()
     if motion=='reduce':
      mid=page.evaluate('mockup.state.camera.position')
      assert abs(mid-1.2)<.00001,(key,'reduced zoom expected 1.2',mid)
     else:
      # The first rAF callback establishes time with dt=0. Wait for a real
      # intermediate position instead of assuming two frames ran in 35 ms.
      poll(page,key,'spring intermediate position 1 < position < 1.2',
       '() => mockup.state.camera.position > 1 && mockup.state.camera.position < 1.2',
       CAMERA,timeout=200)
     page.locator('#zoom-out').click()
     poll(page,key,'reversed zoom settles at 1',CAMERA_HOME,CAMERA,timeout=1500)
     if motion=='no-preference':
      page.locator('#zoom-in').click();page.emulate_media(reduced_motion='reduce')
      poll(page,key,'reduced motion change settles camera',
       '() => mockup.state.camera.position===mockup.state.camera.target && mockup.state.camera.velocity===0',CAMERA)
      page.emulate_media(reduced_motion=motion)
     page.locator('#fit').click()
     poll(page,key,'fit before pan settles at 1',CAMERA_HOME,CAMERA,timeout=1500)
     # Pointer pan continues to work under reduced motion.
     r=page.locator('#map').bounding_box();before=page.evaluate('mockup.project(350,400)')
     page.mouse.move(r['x']+30,r['y']+30);page.mouse.down();page.mouse.move(r['x']+60,r['y']+45,steps=3);page.mouse.up();after=page.evaluate('mockup.project(350,400)');assert abs(after[0]-before[0]-30)<1,(key,'pan expected x delta 30',{'before':before,'after':after})
     page.locator('#fit').click()
     poll(page,key,'fit after pan settles at 1',CAMERA_HOME,CAMERA,timeout=1500)
     # Theme toggle and restore; no settings are stored.
     page.locator('#theme').click();expect(key,'theme toggle',page.locator('html').get_attribute('data-theme'),'dark' if theme=='light' else 'light');page.locator('#theme').click()
     page.locator('#pause').click();expect(key,'resume clock',page.evaluate('mockup.state.paused'),False)
     page.locator('#pause').click();expect(key,'pause clock',page.evaluate('mockup.state.paused'),True)
     # Native result dialog, figure-ground and return focus.
     page.locator('#results').click()
     # Reduced motion disables transitions, but .shown still awaits an opening rAF.
     poll(page,key,'dialog opacity reaches 1',
      '() => getComputedStyle(document.querySelector("#result")).opacity === "1"',DIALOG,timeout=1000)
     expect(key,'result is modal',page.evaluate('document.querySelector("#result").matches(":modal")'),True)
     expect(key,'result metric count',page.locator('.result-metrics > div').count(),3)
     pop=page.evaluate('''()=>{const d=document.querySelector('#result');return {surface:getComputedStyle(d).backgroundColor,scrim:getComputedStyle(d,'::backdrop').backgroundColor,opacity:getComputedStyle(d).opacity}}''')
     alpha=float(re.findall(r'[\d.]+',pop['scrim'])[-1]);worst=[255*(1-alpha)]*3
     popup_ratio=contrast(channels(pop['surface']),worst);assert popup_ratio>=2.5,(key,'popup contrast expected >= 2.5',popup_ratio,pop)
     assert pop['opacity']=='1',(key,'dialog opacity expected 1',pop['opacity'])
     page.screenshot(path=str(ROOT/'shots'/(key+'-dialog.png')))
     page.keyboard.press('Escape')
     poll(page,key,'Escape closes dialog and restores focus',DIALOG_CLOSED,DIALOG)
     expect(key,'result visible after Escape',page.locator('#result').is_visible(),False)
     expect(key,'focus after Escape',page.evaluate('document.activeElement.id'),'results')
     # Closing a popup while opening must not leave a backdrop or trap.
     page.locator('#results').click()
     # Interrupt as soon as modal, without waiting for the opening fade to finish.
     poll(page,key,'dialog becomes modal for interruption',
      '() => document.querySelector("#result").matches(":modal")',DIALOG)
     page.keyboard.press('Escape')
     poll(page,key,'interrupted opening closes and restores focus',DIALOG_CLOSED,DIALOG)
     expect(key,'result open after interrupted opening',page.evaluate('document.querySelector("#result").open'),False)
     assert not errors,(key,errors)
     assert not network,(key,network)
     rows.append({'case':key,'passed':True,'console_errors':len(errors),'external_requests':len(network),'layout':layout,'contrast':ratios,'popup_min_worst_white':round(popup_ratio,3)})
     print('PASS',key,'popup',round(popup_ratio,2),'used',round(layout['used']*100,1),flush=True)
     context.close()
 browser.close()
report={'browser':'Chromium '+version,'date':'2026-11-10','cwd':str(Path.cwd()),'cases':rows,'total':len(rows),'screenshots':len(list((ROOT/'shots').glob('*-dialog.png')))*2,'sha256':{v:hashlib.sha256((ROOT/(v+'.html')).read_bytes()).hexdigest() for v in 'abc'}}
(ROOT/'verification.json').write_text(json.dumps(report,indent=2))
print(f'{len(rows)}/24 passed; zero console errors; zero external requests. Report: {ROOT/"verification.json"}')
