# Instalador visual (apps/installer)

`Conexao-Instalador.exe` é o que o site entrega em `/baixar`. É uma janela pequena no
visual do launcher que:

1. lê `https://api.<domínio>/launcher/latest.json` (a mesma fonte da atualização automática);
2. baixa o instalador NSIS da versão mais nova, mostrando o progresso;
3. confere a assinatura com a chave do atualizador (`tauri.release.conf.json`): se não
   bater, nada é executado;
4. roda o NSIS em modo silencioso (instala em `%LOCALAPPDATA%\Conexao`, sem pedir administrador);
5. cria (ou não) o atalho na área de trabalho e abre o launcher.

O NSIS continua existindo: é ele que a atualização automática do launcher usa, e é o
"instalador clássico" (`/baixar?classico=1`, link nas dúvidas do site) para quem tiver problema.

## Publicar

Nada muda no fluxo: a tag `v*` faz o CI gerar o NSIS e também o `Conexao-Instalador.exe`
(assinado com o Azure Trusted Signing, quando configurado) no mesmo release. O
`atualizar.sh` da VPS baixa os dois, e o `/baixar` passa a entregar o instalador visual.

O instalador visual sempre baixa a versão mais nova: não precisa gerar outro a cada versão
do launcher, mas o CI gera de qualquer jeito (não custa nada e mantém tudo no mesmo release).

## Mexer no visual

```
cd apps/installer
npm install
npm run dev          # abre no navegador com uma instalação simulada (sem compilar Rust)
npx tauri dev        # janela de verdade (no Windows)
```

Requer o WebView2, que já vem no Windows 11 e nas versões atualizadas do Windows 10.
