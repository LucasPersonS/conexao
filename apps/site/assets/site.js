// Site do Conexão: versão do instalador (latest.json), aviso fora do Windows e dica depois do download.
(function () {
  'use strict';

  var ua = navigator.userAgent || '';
  var windows = /Windows/i.test(ua) && !/Windows Phone/i.test(ua);
  if (!windows) {
    var aviso = document.getElementById('aviso-so');
    if (aviso) aviso.hidden = false;
  }

  function mb(bytes) {
    return (bytes / 1048576).toFixed(1).replace('.', ',') + ' MB';
  }

  // Versão e tamanho do instalador publicado (servidos no mesmo domínio pelo Caddy).
  var meta = document.getElementById('meta-download');
  fetch('/launcher/latest.json', { cache: 'no-cache' })
    .then(function (r) { return r.ok ? r.json() : Promise.reject(r.status); })
    .then(function (data) {
      if (!meta || !data || !data.version) return;
      var partes = ['Versão ' + data.version, 'Grátis', 'Windows 10 e 11 (64 bits)'];
      meta.textContent = partes.join(' · ');
      var plat = data.platforms && (data.platforms['windows-x86_64'] || data.platforms['windows-x86_64-nsis']);
      var nome = plat && plat.url ? plat.url.split('/').pop() : '';
      // O /baixar entrega o instalador visual quando ele existe; senão, o NSIS da versão.
      var candidatos = ['Conexao-Instalador.exe'];
      if (/^[\w.-]+\.exe$/.test(nome)) candidatos.push(nome);
      function tamanhoDe(i) {
        if (i >= candidatos.length) return;
        return fetch('/launcher/' + candidatos[i], { method: 'HEAD', cache: 'no-cache' }).then(function (r) {
          var tamanho = Number(r.headers.get('content-length'));
          if (!r.ok || !(tamanho > 0)) return tamanhoDe(i + 1);
          partes.splice(1, 0, mb(tamanho));
          meta.textContent = partes.join(' · ');
        });
      }
      return tamanhoDe(0);
    })
    .catch(function () { /* fica o texto padrão */ });

  // Dica do SmartScreen depois de clicar em Baixar.
  var toast = document.getElementById('toast');
  var timer;
  function fechar() { if (toast) toast.hidden = true; }
  document.querySelectorAll('[data-baixar]').forEach(function (a) {
    a.addEventListener('click', function () {
      if (!toast || !windows) return;
      toast.hidden = false;
      clearTimeout(timer);
      timer = setTimeout(fechar, 15000);
    });
  });
  var x = document.querySelector('.toast-x');
  if (x) x.addEventListener('click', fechar);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') fechar(); });
})();
