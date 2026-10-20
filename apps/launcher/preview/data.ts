/**
 * Dados fictícios da prévia: servidores, páginas PRO, perfis, eventos, comentários.
 * Nomes, números e pessoas são inventados.
 */
import type { PublicProfile } from '@profile/theme'
import type { ServerPage, ServerSummary, UpcomingEvent } from '../src/lib/api'

const icon = (slug: string) => new URL(`./img/${slug}.webp`, import.meta.url).href
const photo = (id: number, w = 1600, h = 500) => `https://picsum.photos/id/${id}/${w}/${h}`

interface Seed {
  slug: string
  name: string
  description: string
  tags: string[]
  banner: number
  players: number
  max: number
  ping: number
  loader?: 'neoforge' | 'forge' | 'fabric'
  minecraft?: string
  featured?: boolean
  pro?: boolean
  central?: boolean
  page?: boolean
  stars: number
  offline?: boolean
  whitelist?: boolean
  size: number
}

const seeds: Seed[] = [
  {
    slug: 'reino-valdoria',
    name: 'Reino de Valdoria',
    description: 'RP medieval com reinos, guildas de ofício, torneios e uma política que pega fogo toda sexta.',
    tags: ['Medieval', 'Roleplay', 'Guildas'],
    banner: 142,
    players: 184,
    max: 300,
    ping: 18,
    featured: true,
    pro: true,
    central: true,
    page: true,
    stars: 1284,
    size: 612_000_000,
  },
  {
    slug: 'nova-aurora',
    name: 'Nova Aurora RP',
    description: 'Cidade viva 24h: empregos, polícia, hospital, facções e eleições para prefeito a cada temporada.',
    tags: ['Cidade', 'Polícia', 'Empregos'],
    banner: 43,
    players: 412,
    max: 500,
    ping: 24,
    featured: true,
    pro: true,
    page: true,
    stars: 2310,
    size: 845_000_000,
  },
  {
    slug: 'vila-serena',
    name: 'Vila Serena SMP',
    description: 'SMP tranquilo e vanilla+, com vila comunitária, feira aos domingos e zero grief.',
    tags: ['SMP', 'Vanilla+', 'Cozy'],
    banner: 164,
    players: 96,
    max: 150,
    ping: 15,
    loader: 'fabric',
    central: true,
    pro: true,
    page: true,
    stars: 742,
    size: 98_000_000,
  },
  {
    slug: 'ilha-esmeralda',
    name: 'Ilha Esmeralda',
    description: 'Sobrevivência em arquipélago: barcos, comércio entre ilhas e ruínas para explorar.',
    tags: ['Sobrevivência', 'Exploração'],
    banner: 162,
    players: 73,
    max: 120,
    ping: 31,
    central: true,
    stars: 389,
    size: 233_000_000,
  },
  {
    slug: 'floresta-ancestral',
    name: 'Floresta Ancestral',
    description: 'Fantasia com magia, druidas, criaturas antigas e uma árvore que guarda a história do servidor.',
    tags: ['Fantasia', 'Magia', 'Roleplay'],
    banner: 28,
    players: 158,
    max: 250,
    ping: 22,
    pro: true,
    stars: 966,
    size: 712_000_000,
  },
  {
    slug: 'cidade-alta',
    name: 'Cidade Alta RP',
    description: 'RP urbano sério: polícia, facções, tribunal e economia controlada pelos jogadores.',
    tags: ['Cidade', 'Polícia', 'Facções'],
    banner: 88,
    players: 287,
    max: 400,
    ping: 27,
    stars: 1550,
    size: 790_000_000,
  },
  {
    slug: 'porto-neblina',
    name: 'Porto Neblina',
    description: 'Piratas, marinha e contrabando num porto envolto em neblina. Navios de verdade, batalhas no mar.',
    tags: ['Piratas', 'Naval', 'Roleplay'],
    banner: 147,
    players: 129,
    max: 200,
    ping: 29,
    central: true,
    stars: 611,
    size: 540_000_000,
  },
  {
    slug: 'velho-oeste',
    name: 'Velho Oeste RP',
    description: 'Faroeste com xerifes, cowboys, assaltos a trem e duelos ao pôr do sol.',
    tags: ['Faroeste', 'Roleplay'],
    banner: 204,
    players: 118,
    max: 200,
    ping: 33,
    stars: 503,
    size: 455_000_000,
  },
  {
    slug: 'dunas-kaar',
    name: 'Dunas de Kaar',
    description: 'Impérios do deserto, caravanas, oásis disputados e uma tempestade que muda o mapa.',
    tags: ['Roleplay', 'Deserto', 'Impérios'],
    banner: 184,
    players: 62,
    max: 100,
    ping: 38,
    loader: 'forge',
    minecraft: '1.20.1',
    stars: 274,
    size: 380_000_000,
  },
  {
    slug: 'fazenda-feliz',
    name: 'Fazenda Feliz SMP',
    description: 'Plante, cozinhe e decore: SMP cozy com estações do ano, festivais e mercado entre vizinhos.',
    tags: ['SMP', 'Cozy', 'Fazenda'],
    banner: 85,
    players: 54,
    max: 100,
    ping: 19,
    loader: 'fabric',
    central: true,
    stars: 318,
    size: 140_000_000,
  },
  {
    slug: 'pico-gelado',
    name: 'Pico Gelado',
    description: 'Hardcore nas montanhas: frio de verdade, expedições em grupo e uma única vida por temporada.',
    tags: ['Aventura', 'Hardcore'],
    banner: 29,
    players: 41,
    max: 80,
    ping: 26,
    central: true,
    stars: 207,
    size: 260_000_000,
  },
  {
    slug: 'estacao-lunar',
    name: 'Estação Lunar',
    description: 'Sci-fi com foguetes, colônias e tecnologia. Em manutenção para a temporada 3.',
    tags: ['Sci-fi', 'Tecnologia'],
    banner: 120,
    players: 0,
    max: 150,
    ping: 0,
    offline: true,
    stars: 158,
    size: 990_000_000,
  },
]

const now = new Date()

export const servers: ServerSummary[] = seeds.map((s, i) => ({
  id: `00000000-0000-4000-8000-${String(i + 1).padStart(12, '0')}`,
  slug: s.slug,
  name: s.name,
  description: s.description,
  address: `jogar.${s.slug.replace(/-/g, '')}.com.br`,
  minecraft: s.minecraft ?? '1.21.1',
  loader: { kind: s.loader ?? 'neoforge', version: s.loader === 'fabric' ? '0.16.9' : s.loader === 'forge' ? '47.3.0' : '21.1.77' },
  tags: s.tags,
  featured: !!s.featured,
  icon_url: icon(s.slug),
  banner_url: photo(s.banner),
  discord_url: 'https://discord.gg/exemplo',
  whitelist_required: s.whitelist ?? true,
  status: {
    online: !s.offline,
    players_online: s.players,
    players_max: s.max,
    latency_ms: s.offline ? null : s.ping,
    checked_at: now.toISOString(),
  },
  current_version: '2.4.1',
  pack_size: s.size,
  allow_offline: false,
  stars: s.stars,
  pro: !!s.pro,
  supportable: !!s.pro,
  whitelist_form_url: null,
  has_page: !!s.page,
  central_partner: !!s.central,
}))

// ------------------------------------------------------------------ páginas PRO

export const pages: Record<string, ServerPage> = {
  'reino-valdoria': {
    active: true,
    page: {
      theme: { colors: ['#1a1208', '#4a2e12', '#c98a2e'], angle: 165, style: 'linear', accent: '#f2c14e' },
      blocks: [
        {
          type: 'text',
          title: 'Bem-vindo a Valdoria',
          body: 'Três reinos, uma coroa vazia. Escolha a sua casa, aprenda um ofício nas guildas e entre para a história do servidor.\n\nTorneio de cavaleiros toda sexta às 21h, no coliseu da capital.',
        },
        { type: 'image', url: photo(142, 1280, 720), caption: 'A capital vista do portão norte' },
        {
          type: 'gallery',
          urls: [photo(116, 640, 400), photo(136, 640, 400), photo(193, 640, 400), photo(76, 640, 400), photo(78, 640, 400), photo(17, 640, 400)],
          caption: 'Construções feitas pelos jogadores na temporada 4',
        },
        {
          type: 'text',
          title: 'Guildas e ofícios',
          body: 'Ferreiros, alquimistas, mercadores, escribas e a Guarda Real. Cada guilda tem missões semanais e uma sede para decorar.',
        },
      ],
    },
  },
  'nova-aurora': {
    active: true,
    page: {
      theme: { colors: ['#0b0614', '#3d0a52', '#d1006f'], angle: 135, style: 'linear', accent: '#ff4fb8' },
      blocks: [
        {
          type: 'text',
          title: 'A cidade nunca dorme',
          body: 'Mais de 40 empregos, prefeitura eleita pelos jogadores e uma polícia que responde a chamados em tempo real.',
        },
        { type: 'image', url: photo(43, 1280, 720), caption: 'Ponte central à noite' },
        { type: 'gallery', urls: [photo(122, 640, 400), photo(57, 640, 400), photo(195, 640, 400)], caption: 'Bairros da temporada 7' },
      ],
    },
  },
  'vila-serena': {
    active: true,
    page: {
      theme: { colors: ['#0b1620', '#2c4c66', '#8fb8d6'], angle: 160, style: 'linear', accent: '#bfe3ff' },
      blocks: [
        { type: 'text', title: 'Devagar e junto', body: 'Sem pressa e sem grief. Feira aos domingos, mutirão de construção uma vez por mês.' },
        { type: 'gallery', urls: [photo(164, 640, 400), photo(188, 640, 400), photo(49, 640, 400), photo(206, 640, 400)], caption: 'A vila em cada estação' },
      ],
    },
  },
}

// ------------------------------------------------------------------ perfis

export const ME = { uuid: 'LucasConexao', name: 'LucasConexao', kind: 'microsoft' as const }

const badge = (id: string, label: string) => ({ id, label })

export const myProfile: PublicProfile = {
  minecraft_uuid: ME.uuid,
  minecraft_name: ME.name,
  display_name: 'Lucas',
  pronouns: 'ele/dele',
  bio: 'Construindo a Conexão. Prefeito aposentado de Nova Aurora e ferreiro em Valdoria nas horas vagas.',
  links: { discord: 'lucas.conexao', youtube: 'https://youtube.com/@exemplo', tiktok: 'https://tiktok.com/@exemplo' },
  theme: {
    background: { kind: 'gradient', colors: ['#0a0f11', '#1d3410', '#b7fe00'], angle: 160 },
    panel: 'escuro',
    accent: '#b7fe00',
    frame: 'brilho',
    font: 'fofa',
    name_effect: 'gradiente',
    name_colors: ['#b7fe00', '#7cc0f7'],
  },
  background_url: null,
  badges: [badge('equipe', 'Equipe Conexão'), badge('central', 'Membro da Central'), badge('plus', 'Conexão+'), badge('pioneiro', 'Pioneiro')],
}

export const profiles: Record<string, PublicProfile> = {
  Aurora_Mel: {
    minecraft_uuid: 'Aurora_Mel',
    minecraft_name: 'Aurora_Mel',
    display_name: 'Aurora',
    pronouns: 'ela/dela',
    bio: 'Criadora de séries de RP medieval. Rainha de Valdoria (temporada 4) e colecionadora de cavalos.',
    links: { youtube: 'https://youtube.com/@exemplo', twitch: 'https://twitch.tv/exemplo', instagram: 'https://instagram.com/exemplo' },
    theme: {
      background: { kind: 'gradient', colors: ['#3b0d2e', '#a8327a', '#f6b0d6'], angle: 145 },
      panel: 'escuro',
      accent: '#ff8fd0',
      frame: 'coracoes',
      font: 'elegante',
      name_effect: 'brilho',
      name_colors: ['#ffd1ea', '#ff8fd0'],
    },
    background_url: null,
    badges: [badge('central', 'Membro da Central'), badge('smp-central', 'SMP da Central'), badge('plus', 'Conexão+'), badge('apoiador', 'Apoiador')],
  },
  Capitao_Breu: {
    minecraft_uuid: 'Capitao_Breu',
    minecraft_name: 'Capitao_Breu',
    display_name: 'Capitão Breu',
    pronouns: null,
    bio: 'Dono do Porto Neblina. Procuro marujos, cartógrafos e um bom papagaio.',
    links: { discord: 'capitao.breu', tiktok: 'https://tiktok.com/@exemplo' },
    theme: {
      background: { kind: 'gradient', colors: ['#03141c', '#0b4d63', '#1592a8'], angle: 170 },
      panel: 'escuro',
      accent: '#5fd4e8',
      frame: 'pixel',
      font: 'pirata',
      name_effect: 'onda',
      name_colors: ['#9ff0ff', '#5fd4e8'],
    },
    background_url: null,
    badges: [badge('parceiro', 'Dono de servidor'), badge('smp-central', 'SMP da Central'), badge('central', 'Membro da Central')],
  },
  NeonByte: {
    minecraft_uuid: 'NeonByte',
    minecraft_name: 'NeonByte',
    display_name: 'NeonByte',
    pronouns: 'ele/dele',
    bio: 'Speedrunner de modpack, piloto de foguete na Estação Lunar e editor de vídeo.',
    links: { youtube: 'https://youtube.com/@exemplo', x: 'https://x.com/exemplo' },
    theme: {
      background: { kind: 'gradient', colors: ['#0b0614', '#2b0a6e', '#00e5ff'], angle: 120 },
      panel: 'escuro',
      accent: '#00e5ff',
      frame: 'neon',
      font: 'futurista',
      name_effect: 'glitch',
      name_colors: ['#00e5ff', '#d1006f'],
    },
    background_url: null,
    badges: [badge('plus', 'Conexão+'), badge('pioneiro', 'Pioneiro')],
  },
}

// ------------------------------------------------------------------ eventos, notícias e comentários

const inDays = (d: number, h: number) => {
  const date = new Date(now)
  date.setDate(date.getDate() + d)
  date.setHours(h, 0, 0, 0)
  return date.toISOString()
}

export const upcoming: UpcomingEvent[] = [
  { id: 'e1', title: 'Torneio de cavaleiros', body: 'Inscrições no coliseu.', starts_at: inDays(1, 21), slug: 'reino-valdoria', server_name: 'Reino de Valdoria', icon_url: icon('reino-valdoria') },
  { id: 'e2', title: 'Eleição para prefeito', body: 'Debate às 20h.', starts_at: inDays(2, 20), slug: 'nova-aurora', server_name: 'Nova Aurora RP', icon_url: icon('nova-aurora') },
  { id: 'e3', title: 'Tour dos SMPs da Central', body: 'Visita guiada com a equipe.', starts_at: inDays(3, 20), slug: 'vila-serena', server_name: 'Vila Serena SMP', icon_url: icon('vila-serena') },
  { id: 'e4', title: 'Regata das ilhas', body: 'Prêmio para os três primeiros barcos.', starts_at: inDays(5, 19), slug: 'ilha-esmeralda', server_name: 'Ilha Esmeralda', icon_url: icon('ilha-esmeralda') },
]

export const news = (slug: string) => ({
  news: [
    {
      id: `${slug}-n1`,
      kind: 'news' as const,
      title: 'Temporada nova no ar',
      body: 'Mapa novo, 30 mods atualizados e o modpack já baixa sozinho pelo Conexão.',
      starts_at: null,
      created_at: inDays(-1, 18),
      author_name: 'Capitao_Breu',
    },
  ],
  events: upcoming.filter((e) => e.slug === slug).map((e) => ({ ...e, kind: 'event' as const, created_at: inDays(-2, 12), author_name: 'Aurora_Mel' })),
})

const commenters = [
  ['Aurora_Mel', 'Melhor servidor de RP que já joguei. A equipe responde em minutos.'],
  ['NeonByte', 'Entrei pelo Conexão e o modpack instalou sozinho, em 1 minuto já estava na cidade.'],
  ['Capitao_Breu', 'Vim pela Central SMP’s e não saio mais. Evento de sexta é sensacional.'],
] as const

export const comments = (slug: string) => ({
  total: 128,
  stars: servers.find((s) => s.slug === slug)?.stars ?? 0,
  comments: commenters.map(([name, body], i) => ({
    id: `${slug}-c${i}`,
    body,
    created_at: inDays(-i - 1, 15),
    author_name: name,
    author_uuid: name,
    author_role: i === 2 ? ('owner' as const) : null,
    author_offline: false,
    mine: false,
    can_delete: false,
  })),
  has_more: true,
  viewer: { can_comment: true, reason: null, can_moderate: false },
})
