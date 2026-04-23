// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025-2026 Sky, LLC.
// The behaviour every handbook page shares, in ONE place: the glossary's
// way back, the copy buttons, the landing glow. A page adds only what is
// its own (the handbook: search and the rail). Timing lives in the
// stylesheet's --glow-time token; nothing here carries a duration.
(function(){
  function land(id){
    var el=document.getElementById(id);if(!el)return;
    el.classList.remove('landed');void el.offsetWidth;el.classList.add('landed');
    el.addEventListener('animationend',function(){el.classList.remove('landed');},{once:true});
  }
  window.ivLand=land;
  document.addEventListener('click',function(e){
    var a=e.target.closest('a.term');
    if(a){var b=document.querySelector('a.back[data-back="'+a.dataset.key+'"]');if(b)b.setAttribute('href','#'+a.id);return;}
    var c=e.target.closest('.copy');
    if(c){var pre=c.parentNode.querySelector('pre');
      if(pre&&navigator.clipboard){navigator.clipboard.writeText(pre.innerText).then(function(){
        c.textContent='copied';c.classList.add('done');
        setTimeout(function(){c.textContent='copy';c.classList.remove('done');},1400);});}}
  });
})();
