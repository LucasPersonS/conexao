# Prévia do launcher (sem Tauri e sem API)

Roda a interface do launcher no navegador com dados **fictícios**, para tirar prints de
divulgação (home com vários servidores online, páginas PRO personalizadas, perfis, selos da
parceria com a Central SMP's…). Nada aqui vai para o app de verdade.

```bash
cd apps/launcher
npm run preview:mock          # abre em http://localhost:1430
```

Cenas (parâmetro `?cena=` na URL):

| Cena | O que mostra |
| --- | --- |
| `inicio` (padrão) | Home com servidores online, destaque e a faixa "SMPs da Central" |
| `servidores` | Lista de servidores |
| `central` | Lista filtrada só com os SMPs da Central |
| `servidor:<slug>` | Página de um servidor (ex.: `servidor:reino-valdoria`, com página PRO) |
| `perfil` | Editor do meu perfil |
| `perfil:<nick>` | Janela de perfil de outro jogador (ex.: `perfil:Aurora_Mel`) |
| `loja` | Loja |

Os dados ficam em `preview/data.ts`. Imagens: banners do [Lorem Picsum](https://picsum.photos)
(fotos do Unsplash, licença Unsplash), ícones montados com emojis do
[Twemoji](https://github.com/jdecked/twemoji) (CC-BY 4.0) em `preview/img/`, cabeças de skin do
[mc-heads.net](https://mc-heads.net). Servidores, jogadores e números são inventados.
