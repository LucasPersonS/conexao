// Site do Conexão: lista de espera (login pelo Discord), download para quem foi liberado,
// contador da comunidade no Discord, versão do instalador e dica depois do download.
(function () {
  'use strict';

  var ua = navigator.userAgent || '';
  var windows = /Windows/i.test(ua) && !/Windows Phone/i.test(ua);
  var lista = document.getElementById('lista');
  var erroBox = document.getElementById('lista-erro');
  var form = document.getElementById('form-pedido');
  var topoCta = document.getElementById('topo-cta');
  var atual = null;
  var editando = false;

  function $all(sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); }
  function numero(n) { return Number(n || 0).toLocaleString('pt-BR'); }

  function mostrarErro(texto) {
    if (!erroBox) return;
    erroBox.textContent = texto || '';
    erroBox.hidden = !texto;
  }

  function mostrar(estado) {
    $all('.estado', lista).forEach(function (el) { el.hidden = el.getAttribute('data-estado') !== estado; });
  }

  // Erros que voltam do Discord (?erro=...): mostra o aviso e limpa a URL.
  var ERROS = {
    cancelado: 'O login no Discord foi cancelado. Tente de novo quando quiser.',
    discord: 'Não conseguimos confirmar a sua conta do Discord. Tente de novo.',
    indisponivel: 'O login pelo Discord ainda não está disponível. Volte daqui a pouco.'
  };
  var params = new URLSearchParams(location.search);
  if (params.has('erro')) {
    mostrarErro(ERROS[params.get('erro')] || ERROS.discord);
    history.replaceState(null, '', location.pathname + location.hash);
  }

  // Link de origem (?ref=central): guardado no navegador e repassado ao login do Discord,
  // para a API registrar de onde a pessoa veio (e conferir se é membro da Central).
  var REF_KEY = 'conexao_ref';
  var refAtual = null;
  try { refAtual = localStorage.getItem(REF_KEY); } catch (e) { /* sem storage */ }
  var refUrl = (params.get('ref') || '').toLowerCase();
  if (/^[a-z0-9-]{1,32}$/.test(refUrl)) {
    refAtual = refUrl;
    try { localStorage.setItem(REF_KEY, refUrl); } catch (e) { /* sem storage */ }
    params.delete('ref');
    var resto = params.toString();
    history.replaceState(null, '', location.pathname + (resto ? '?' + resto : '') + location.hash);
  }
  $all('[data-login]').forEach(function (a) {
    if (refAtual) a.setAttribute('href', '/v1/waitlist/discord?ref=' + encodeURIComponent(refAtual));
  });

  function mostrarCentral(me) {
    var veio = (me && me.ref === 'central') || refAtual === 'central';
    var membro = !!(me && me.central);
    $all('[data-via-central]').forEach(function (el) { el.hidden = !veio || membro || !!(me && me.can_download); });
    $all('[data-via-central-texto]').forEach(function (el) {
      el.textContent = me && me.discord
        ? 'Não encontramos a sua conta no servidor da Central. Entre na Central e faça login de novo para entrar no lote prioritário.'
        : 'Entre com o Discord da sua conta na Central: membros verificados entram no lote prioritário.';
    });
    $all('[data-selo-central]').forEach(function (el) { el.hidden = !membro; });
    $all('[data-central-convite]').forEach(function (el) {
      if (me && me.central_invite) { el.setAttribute('href', me.central_invite); el.hidden = false; }
    });
  }

  function preencher(me) {
    var d = me.discord;
    $all('[data-nome]').forEach(function (el) { el.textContent = d ? d.name : ''; });
    $all('[data-avatar]').forEach(function (el) {
      if (d && d.avatar_url) el.src = d.avatar_url;
    });
    $all('[data-fila]').forEach(function (el) {
      el.textContent = me.waiting > 0 ? numero(me.waiting) + (me.waiting === 1 ? ' pessoa' : ' pessoas') : 'Pouca gente';
    });
    $all('[data-selo]').forEach(function (el) { el.hidden = !me.staff; });
    $all('[data-painel]').forEach(function (el) { el.hidden = !me.staff; });
    var e = me.entry;
    $all('[data-nick]').forEach(function (el) { el.textContent = e ? e.minecraft_name : ''; });
    $all('[data-posicao]').forEach(function (el) { el.textContent = e && e.position ? '#' + numero(e.position) : '#–'; });
    var titulo = lista.querySelector('[data-liberado-titulo]');
    if (titulo) titulo.textContent = me.staff ? 'Acesso da equipe liberado' : 'Você foi liberado!';
  }

  function render(me) {
    atual = me;
    preencher(me);
    mostrarCentral(me);
    if (topoCta) {
      topoCta.textContent = me.can_download ? 'Baixar' : 'Entrar na lista';
      topoCta.setAttribute('href', me.can_download ? '/baixar' : '#lista');
      if (me.can_download) topoCta.setAttribute('data-baixar', ''); else topoCta.removeAttribute('data-baixar');
    }
    if (me.can_download) return mostrar('liberado');
    if (!me.discord) return mostrar('entrar');
    var e = me.entry;
    if (!e || editando) {
      if (e && form) {
        form.nick.value = e.minecraft_name;
        $all('input[name=original]', form).forEach(function (r) { r.checked = (r.value === '1') === e.original; });
      }
      return mostrar('pedido');
    }
    if (e.status === 'rejected') return mostrar('recusado');
    return mostrar('fila');
  }

  function api(method, body) {
    return fetch('/v1/waitlist/me', {
      method: method,
      credentials: 'same-origin',
      headers: body ? { 'Content-Type': 'application/json' } : {},
      body: body ? JSON.stringify(body) : undefined
    }).then(function (r) {
      return r.json().catch(function () { return {}; }).then(function (data) {
        if (!r.ok) throw new Error(data.message || 'Algo deu errado. Tente de novo.');
        return data;
      });
    });
  }

  function carregar() {
    return api('GET').then(render).catch(function () {
      mostrar('entrar');
      mostrarCentral(null);
      mostrarErro('A lista de espera está fora do ar agora. Tente de novo em instantes.');
    });
  }

  if (lista) carregar();

  if (form) {
    form.addEventListener('submit', function (ev) {
      ev.preventDefault();
      mostrarErro('');
      var nick = form.nick.value.trim();
      if (!/^[A-Za-z0-9_]{3,16}$/.test(nick)) {
        mostrarErro('O nick precisa ter de 3 a 16 caracteres: letras, números e _.');
        form.nick.focus();
        return;
      }
      var original = (form.querySelector('input[name=original]:checked') || {}).value !== '0';
      var botao = document.getElementById('enviar-pedido');
      botao.disabled = true;
      botao.textContent = original ? 'Conferindo o nick…' : 'Enviando…';
      api('POST', { nick: nick, original: original })
        .then(function (me) { editando = false; render(me); })
        .catch(function (err) { mostrarErro(err.message); })
        .then(function () { botao.disabled = false; botao.textContent = 'Garantir meu lugar'; });
    });
  }

  document.addEventListener('click', function (ev) {
    var alvo = ev.target.closest ? ev.target.closest('[data-sair],[data-editar]') : null;
    if (!alvo) return;
    if (alvo.hasAttribute('data-editar')) {
      editando = true;
      if (atual) render(atual);
      if (form) form.nick.focus();
      return;
    }
    fetch('/v1/waitlist/sair', { method: 'POST', credentials: 'same-origin' })
      .catch(function () {})
      .then(function () { editando = false; mostrarErro(''); carregar(); });
  });

  // Contador da comunidade no Discord (pelo convite público; some se não vier).
  var numeros = document.querySelector('[data-comunidade]');
  if (numeros) {
    fetch('/v1/waitlist/comunidade', { credentials: 'same-origin' })
      .then(function (r) { return r.ok ? r.json() : Promise.reject(r.status); })
      .then(function (c) {
        if (!c || !c.members) return;
        numeros.querySelector('[data-membros]').textContent = numero(c.members);
        numeros.querySelector('[data-online]').textContent = numero(c.online || 0);
        numeros.hidden = false;
      })
      .catch(function () { /* fica sem números */ });
  }

  // Fora do Windows: avisa no lugar do download.
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
  document.addEventListener('click', function (ev) {
    var a = ev.target.closest ? ev.target.closest('[data-baixar]') : null;
    if (!a || !toast || !windows) return;
    toast.hidden = false;
    clearTimeout(timer);
    timer = setTimeout(fechar, 15000);
  });
  var x = document.querySelector('.toast-x');
  if (x) x.addEventListener('click', fechar);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') fechar(); });
})();
