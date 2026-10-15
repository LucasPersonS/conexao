//! Filtro de texto dos perfis (nome, pronomes, bio).
//!
//! Muitos jogadores são menores: palavras ofensivas e links fora dos campos
//! próprios são recusados. A lista base cobre o óbvio em português e inglês; a
//! equipe pode acrescentar termos em `CONEXAO_BLOCKED_WORDS_FILE` (um por linha).
//! O filtro não substitui as denúncias: ele só barra o grosseiro.

use std::collections::HashSet;
use std::sync::OnceLock;

const BASE_WORDS: &[&str] = &[
    // ofensas e discriminação
    "viado", "viadinho", "bicha", "traveco", "sapatao", "macaco", "crioulo", "retardado", "mongoloide",
    "faggot", "fag", "nigger", "nigga", "retard", "tranny", "chink", "kike", "spic",
    // conteúdo sexual explícito
    "porno", "porn", "xvideos", "pornhub", "onlyfans", "nudes", "putaria", "buceta", "piroca", "pau no cu",
    "hentai", "rule34",
    // incentivo a violência / ódio
    "nazista", "nazi", "hitler", "kys", "se mata", "kill yourself", "heil",
];

fn extra_words() -> &'static HashSet<String> {
    static EXTRA: OnceLock<HashSet<String>> = OnceLock::new();
    EXTRA.get_or_init(|| {
        crate::config::var("CONEXAO_BLOCKED_WORDS_FILE")
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| {
                text.lines()
                    .map(normalize)
                    .filter(|w| !w.is_empty() && !w.starts_with('#'))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Minúsculas, sem acentos, "leet" desfeito (0→o, 1→i, 3→e, 4→a, 5→s, 7→t, @→a, $→s),
/// letras repetidas colapsadas e separadores viram espaço.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = ' ';
    for c in text.to_lowercase().chars() {
        let mapped = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | '4' | '@' => 'a',
            'é' | 'è' | 'ê' | 'ë' | '3' => 'e',
            'í' | 'ì' | 'î' | 'ï' | '1' | '!' | '|' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | '0' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            '5' | '$' => 's',
            '7' => 't',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        if mapped == last && mapped != ' ' {
            continue;
        }
        if mapped == ' ' && last == ' ' {
            continue;
        }
        out.push(mapped);
        last = mapped;
    }
    out.trim().to_string()
}

fn collapse(word: &str) -> String {
    let mut out = String::new();
    let mut last = '\0';
    for c in word.chars() {
        if c != last {
            out.push(c);
        }
        last = c;
    }
    out
}

/// Devolve o termo encontrado, se houver.
pub fn blocked_term(text: &str) -> Option<String> {
    let normalized = normalize(text);
    let padded = format!(" {normalized} ");
    let squashed: String = normalized.chars().filter(|c| *c != ' ').collect();
    let words = BASE_WORDS.iter().map(|w| collapse(&normalize(w))).chain(extra_words().iter().cloned());
    for word in words {
        if word.is_empty() {
            continue;
        }
        // Palavra inteira (evita falso positivo dentro de outras palavras)…
        if padded.contains(&format!(" {word} ")) {
            return Some(word);
        }
        // …ou escondida com espaços/pontos ("p o r n"), só para termos longos.
        if word.len() >= 5 && !word.contains(' ') && squashed.contains(&word) {
            return Some(word);
        }
    }
    None
}

/// Bio e nomes não podem ter links (eles têm campos próprios, com domínios conferidos).
pub fn has_link(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("www.")
        || lower.contains(".gg/")
        || lower.contains(".com/")
        || lower.contains(".net/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tricks() {
        assert_eq!(normalize("P0RRN!!"), "porni");
        assert_eq!(normalize("Ação   Já"), "acao ja");
    }

    #[test]
    fn blocks_obvious_terms() {
        assert!(blocked_term("meu onlyfans é tal").is_some());
        assert!(blocked_term("p.o.r.n.h.u.b").is_some());
        assert!(blocked_term("N4ZI").is_some());
        assert!(blocked_term("seu retardado").is_some());
    }

    #[test]
    fn allows_normal_text() {
        assert!(blocked_term("Amo RP medieval, cavalos e magia. Ela/dela 💜").is_none());
        assert!(blocked_term("Sou macacão de obra no RP, sapateiro e escultor").is_none());
        assert!(blocked_term("Grass, Snapshots").is_none());
        assert!(blocked_term("x").is_none());
        assert!(blocked_term("Me segue no X").is_none());
    }

    #[test]
    fn detects_links() {
        assert!(has_link("me chama em discord.gg/abc"));
        assert!(has_link("https://x.com"));
        assert!(!has_link("Ela/dela · 19 anos de Minecraft"));
    }
}
