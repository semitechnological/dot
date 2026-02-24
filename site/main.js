(function () {
  var SCRIPT_URL = 'https://cdn.jsdelivr.net/gh/hiunicornstudio/unicornstudio.js@v2.0.5/dist/unicornStudio.umd.js';
  var CACHE_KEY = 'us-script-v2.0.5';

  function boot() {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function () { UnicornStudio.init(); });
    } else {
      UnicornStudio.init();
    }
  }

  var u = window.UnicornStudio;
  if (u && u.init) { boot(); return; }

  window.UnicornStudio = { isInitialized: false };

  if (typeof caches !== 'undefined') {
    caches.open(CACHE_KEY).then(function (cache) {
      return cache.match(SCRIPT_URL).then(function (resp) {
        if (resp) return resp;
        return fetch(SCRIPT_URL).then(function (networkResp) {
          cache.put(SCRIPT_URL, networkResp.clone());
          return networkResp;
        });
      }).then(function (resp) {
        return resp.text();
      }).then(function (code) {
        var s = document.createElement('script');
        s.textContent = code;
        document.head.appendChild(s);
        boot();
      });
    }).catch(loadFallback);
  } else {
    loadFallback();
  }

  function loadFallback() {
    var s = document.createElement('script');
    s.src = SCRIPT_URL;
    s.onload = boot;
    (document.head || document.body).appendChild(s);
  }
})();

(function () {
  var tag = document.getElementById('version-tag');
  var CACHE_TTL = 1000 * 60 * 30;
  var STORAGE_KEY = 'dot-latest-version';

  var cached = null;
  try {
    var raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      cached = JSON.parse(raw);
      if (Date.now() - cached.ts < CACHE_TTL) {
        tag.textContent = cached.v;
        return;
      }
    }
  } catch (_) {}

  fetch('https://api.github.com/repos/plyght/dot/releases/latest')
    .then(function (r) {
      if (!r.ok) throw new Error(r.status);
      return r.json();
    })
    .then(function (data) {
      var v = data.tag_name;
      if (v) {
        if (v[0] !== 'v') v = 'v' + v;
        tag.textContent = v;
        try { localStorage.setItem(STORAGE_KEY, JSON.stringify({ v: v, ts: Date.now() })); } catch (_) {}
      }
    })
    .catch(function () {
      fetch('https://api.github.com/repos/plyght/dot/tags?per_page=1')
        .then(function (r) { return r.json(); })
        .then(function (tags) {
          if (tags.length) {
            var v = tags[0].name;
            if (v[0] !== 'v') v = 'v' + v;
            tag.textContent = v;
            try { localStorage.setItem(STORAGE_KEY, JSON.stringify({ v: v, ts: Date.now() })); } catch (_) {}
          }
        })
        .catch(function () {});
    });
})();

(function () {
  var btn = document.getElementById('about-btn');
  var backdrop = document.getElementById('modal-backdrop');
  var modal = document.getElementById('modal');
  var close = document.getElementById('modal-close');

  function open() { backdrop.classList.add('open'); }
  function shut() { backdrop.classList.remove('open'); }

  btn.addEventListener('click', open);
  close.addEventListener('click', shut);
  backdrop.addEventListener('click', shut);
  modal.addEventListener('click', function (e) { e.stopPropagation(); });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') shut();
  });

  var copyBtn = document.getElementById('modal-copy-btn');
  var copyIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>';
  var checkIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>';

  function copyText(text) {
    try {
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.style.cssText = 'position:fixed;opacity:0';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    } catch (_) {}
    if (navigator.clipboard) navigator.clipboard.writeText(text).catch(function () {});
  }

  copyBtn.addEventListener('click', function () {
    copyText('cargo install dot-ai');
    copyBtn.innerHTML = checkIcon;
    copyBtn.classList.add('copied');
    setTimeout(function () {
      copyBtn.innerHTML = copyIcon;
      copyBtn.classList.remove('copied');
    }, 1500);
  });
})();
