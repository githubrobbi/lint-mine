// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025-2026 Sky, LLC.
// The theme switch and the text size shared by every handbook page:
// system theme by default, a choice stamped on the root and remembered
// per browser; the text size as a zoom of the page alone (never the
// browser's, which reaches every tab), in steps, remembered the same
// way. Both run before paint so a remembered choice never flashes the
// other. The tooltips say what is active and what the click does next.
// (Carried over from docenta's page layer as of 2026-10-11; D86.)
(function(){
  var root=document.documentElement;
  var KEY='iv-theme';
  function saved(){try{return localStorage.getItem(KEY)||'';}catch(e){return '';}}
  function apply(v){if(v==='light'||v==='dark'){root.setAttribute('data-theme',v);}else{root.removeAttribute('data-theme');}}
  apply(saved());
  var SKEY='iv-size',SIZES=[85,100,115,130,150,175,200];
  function sizeSaved(){try{var n=parseInt(localStorage.getItem(SKEY),10);return SIZES.indexOf(n)>=0?n:100;}catch(e){return 100;}}
  function sizeApply(n){root.style.setProperty('--zoom',String(n/100));}
  function sizeStore(n){try{if(n===100){localStorage.removeItem(SKEY);}else{localStorage.setItem(SKEY,String(n));}}catch(e){}}
  sizeApply(sizeSaved());
  document.addEventListener('DOMContentLoaded',function(){
    var b=document.getElementById('theme');
    if(b){
      var order=['','light','dark'],names={'':'System','light':'Light','dark':'Dark'};
      function systemNow(){return window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';}
      function label(){
        var v=saved(),i=order.indexOf(v),next=order[(i+1)%3],after=order[(i+2)%3];
        var now=v?names[v]:'System, '+systemNow()+' right now';
        var tip='Theme: '+now+'. Click for '+names[next]+', then '+names[after]+'.';
        b.title=tip;b.setAttribute('aria-label',tip);b.dataset.mode=v||'system';
      }
      label();
      b.addEventListener('click',function(){
        var v=saved(),next=order[(order.indexOf(v)+1)%order.length];
        try{if(next){localStorage.setItem(KEY,next);}else{localStorage.removeItem(KEY);}}catch(e){}
        apply(next);label();
      });
      if(window.matchMedia){try{window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change',label);}catch(e){}}
    }
    var up=document.getElementById('size-up'),down=document.getElementById('size-down'),show=document.getElementById('size');
    if(up&&down&&show){
      function sizeLabel(){
        var n=sizeSaved(),i=SIZES.indexOf(n);
        show.textContent=n+'%';
        show.title=(n===100?'Text size: 100%, the page as written.':'Text size: '+n+'%. Click to go back to 100%.')+' Command (Control on Windows) with + or - steps it here, 0 resets; the browser\'s own zoom is left alone.';
        show.setAttribute('aria-label',show.title);
        down.disabled=i<=0;up.disabled=i>=SIZES.length-1;
        down.title=down.disabled?'Smallest text already':'Smaller text, '+SIZES[i-1]+'%';
        up.title=up.disabled?'Largest text already':'Larger text, '+SIZES[i+1]+'%';
        down.setAttribute('aria-label',down.title);up.setAttribute('aria-label',up.title);
      }
      function set(n){sizeStore(n);sizeApply(n);sizeLabel();}
      function step(d){var i=SIZES.indexOf(sizeSaved())+d;if(i>=0&&i<SIZES.length){set(SIZES[i]);}}
      sizeLabel();
      up.addEventListener('click',function(){step(1);});
      down.addEventListener('click',function(){step(-1);});
      show.addEventListener('click',function(){set(100);});
      // Command (Control on Windows) with + - 0: on this page these size
      // THIS page, and the browser's zoom, which reaches every tab, is not
      // invoked (the event is taken before the browser acts).
      document.addEventListener('keydown',function(e){
        if(!(e.metaKey||e.ctrlKey)||e.altKey||e.shiftKey&&e.code!=='Equal')return;
        if(e.code==='Equal'||e.code==='NumpadAdd'){step(1);e.preventDefault();}
        else if(e.code==='Minus'||e.code==='NumpadSubtract'){step(-1);e.preventDefault();}
        else if(e.code==='Digit0'||e.code==='Numpad0'){set(100);e.preventDefault();}
      });
    }
  });
})();
