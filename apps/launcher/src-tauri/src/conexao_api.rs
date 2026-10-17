//! Cliente da API central do Conexão.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use conexao_types::{AccessResponse, JoinTicketResponse, PackManifest, ServerSummary};
use serde::{Deserialize, Serialize};

use crate::AppContext;
use crate::account::{OfflineAccount, default_credentials, load_offline, remove_offline, save_offline};
use crate::error::{CmdError, CmdResult};

/// URL da API, definida no build (`CONEXAO_API_URL`). Em desenvolvimento também pode
/// ser trocada na execução pela variável de ambiente de mesmo nome.
const DEFAULT_API: &str = match option_env!("CONEXAO_API_URL") {
    Some(url) => url,
    None => "https://api.conexao.gg",
};

/// Chave pública Ed25519 dos manifestos, definida no build
/// (`CONEXAO_MANIFEST_PUBKEY`, gerada com `conexao-api gerar-chave-assinatura`).
/// Com ela, o launcher recusa qualquer manifesto sem assinatura válida.
/// O build de produção falha sem ela (`build.rs`).
const MANIFEST_PUBLIC_KEY: Option<&str> = option_env!("CONEXAO_MANIFEST_PUBKEY");

fn manifest_public_key() -> Option<&'static str> {
    MANIFEST_PUBLIC_KEY.map(str::trim).filter(|k| !k.is_empty())
}

pub fn verify_manifest(public_key: Option<&str>, body: &[u8], signature: Option<&str>) -> CmdResult<()> {
    let Some(public_key) = public_key else {
        // Só um build de desenvolvimento pode rodar sem a chave.
        if cfg!(debug_assertions) {
            return Ok(());
        }
        return Err(CmdError::new("Este launcher foi gerado sem a chave de segurança dos modpacks. Baixe a versão oficial."));
    };
    let signature = signature.ok_or_else(|| CmdError::new("O modpack veio sem assinatura do Conexão. Instalação bloqueada."))?;
    conexao_types::signing::verify(public_key, body, signature)
        .map_err(|_| CmdError::new("A assinatura do modpack não confere. Instalação bloqueada por segurança."))
}

pub struct ConexaoApi {
    base: String,
    http: reqwest::Client,
    token: Mutex<Option<String>>,
    /// Pasta de dados do launcher (definida no `setup` do Tauri).
    data_dir: OnceLock<PathBuf>,
    /// Conta não original ativa, se houver (tem prioridade sobre a conta Microsoft).
    offline: Mutex<Option<OfflineAccount>>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConexaoUser {
    pub id: uuid::Uuid,
    pub minecraft_uuid: uuid::Uuid,
    pub minecraft_name: String,
    pub is_staff: bool,
    #[serde(default)]
    pub account_type: String,
}

#[derive(Deserialize)]
pub struct DiscordConfig {
    pub enabled: bool,
    pub client_id: Option<String>,
    pub redirect_uri: String,
    pub scope: String,
}

#[derive(Deserialize)]
pub struct OfflineLogin {
    pub token: String,
    pub session: String,
    pub user: ConexaoUser,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DiscordLoginResponse {
    Ok(OfflineLogin),
    NeedsNick { signup_token: String, discord_name: String, suggested_nick: Option<String> },
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
    user: ConexaoUser,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
}

#[derive(Deserialize, Serialize)]
pub struct PairingInfo {
    pub ip: String,
    pub browser: String,
    pub age_secs: u64,
}

#[derive(Deserialize)]
pub struct Membership {
    pub slug: String,
    pub name: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct MeResponse {
    pub user: ConexaoUser,
    pub memberships: Vec<Membership>,
}

/// O que o jogador administra: decide se o menu "Administração" aparece.
#[derive(Serialize)]
pub struct MyRoles {
    pub is_staff: bool,
    pub servers: Vec<ManagedServer>,
}

#[derive(Serialize)]
pub struct ManagedServer {
    pub slug: String,
    pub name: String,
    pub role: String,
}

pub enum ManifestFetch {
    NotModified,
    Manifest { manifest: Box<PackManifest>, etag: Option<String> },
}

impl ConexaoApi {
    pub fn from_env() -> Self {
        let runtime = std::env::var("CONEXAO_API_URL").ok().filter(|_| cfg!(debug_assertions));
        let base = runtime.unwrap_or_else(|| DEFAULT_API.to_string());
        let http = reqwest::Client::builder()
            .user_agent(concat!("Conexao-Launcher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            // Nenhuma resposta da API demora isso; evita o launcher travado esperando para sempre.
            .timeout(Duration::from_secs(60))
            .https_only(!cfg!(debug_assertions))
            .build()
            .unwrap_or_default();
        Self {
            base: base.trim_end_matches('/').to_string(),
            http,
            token: Mutex::new(None),
            data_dir: OnceLock::new(),
            offline: Mutex::new(None),
        }
    }

    /// Chamado uma vez na inicialização: carrega a conta não original salva.
    pub fn set_data_dir(&self, dir: PathBuf) {
        if let Ok(mut offline) = self.offline.lock() {
            *offline = load_offline(&dir);
        }
        let _ = self.data_dir.set(dir);
    }

    pub fn offline_account(&self) -> Option<OfflineAccount> {
        self.offline.lock().ok().and_then(|o| o.clone())
    }

    fn set_token(&self, value: String) {
        if let Ok(mut token) = self.token.lock() {
            *token = Some(value);
        }
    }

    /// Salva a conta não original recém-logada e já guarda o JWT dela.
    pub async fn store_offline_login(&self, login: OfflineLogin) -> CmdResult<OfflineAccount> {
        let account =
            OfflineAccount { uuid: login.user.minecraft_uuid, name: login.user.minecraft_name, session: login.session };
        let dir = self.data_dir.get().ok_or_else(|| CmdError::new("launcher ainda iniciando"))?;
        save_offline(dir, &account)?;
        if let Ok(mut offline) = self.offline.lock() {
            *offline = Some(account.clone());
        }
        self.set_token(login.token);
        // Uma conta por vez: a Microsoft salva deixa de ser usada.
        if let Ok(Some(user)) = theseus::minecraft_auth::get_default_user().await {
            let _ = theseus::minecraft_auth::remove_user(user).await;
        }
        Ok(account)
    }

    /// Sai da conta não original: apaga a sessão no Conexão (se der) e no computador.
    pub async fn forget_offline(&self) {
        let account = self.offline.lock().ok().and_then(|mut o| o.take());
        if let Some(account) = account {
            let _ = self
                .http
                .post(format!("{}/v1/auth/offline/logout", self.base))
                .json(&serde_json::json!({ "session": account.session }))
                .send()
                .await;
        }
        if let Some(dir) = self.data_dir.get() {
            remove_offline(dir);
        }
    }

    pub async fn discord_config(&self) -> CmdResult<DiscordConfig> {
        let response = self.http.get(format!("{}/v1/auth/discord/config", self.base)).send().await?;
        Ok(Self::check(response).await?.json().await?)
    }

    pub async fn discord_login(&self, code: &str) -> CmdResult<DiscordLoginResponse> {
        let response = self
            .http
            .post(format!("{}/v1/auth/discord", self.base))
            .json(&serde_json::json!({ "code": code }))
            .send()
            .await?;
        Ok(Self::check(response).await?.json().await?)
    }

    pub async fn register_offline(&self, signup_token: &str, nick: &str) -> CmdResult<OfflineLogin> {
        let response = self
            .http
            .post(format!("{}/v1/auth/offline/register", self.base))
            .json(&serde_json::json!({ "signup_token": signup_token, "nick": nick }))
            .send()
            .await?;
        Ok(Self::check(response).await?.json().await?)
    }

    /// Passe de entrada para servidores que aceitam contas não originais. Vai pelo
    /// endereço só IPv4 informado pela API, para o IP registrado ser o mesmo do jogo.
    pub async fn join_ticket(&self, join_url: &str, slug: &str) -> CmdResult<JoinTicketResponse> {
        let join_url = join_url.trim_end_matches('/');
        if !(join_url.starts_with("https://") || cfg!(debug_assertions)) {
            return Err(CmdError::new("Endereço do passe de entrada inválido."));
        }
        let request = self.http.post(format!("{join_url}/v1/servers/{slug}/join-ticket"));
        Ok(self.authed(request).await?.json().await?)
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn clear_token(&self) {
        if let Ok(mut token) = self.token.lock() {
            *token = None;
        }
    }

    pub fn token(&self) -> Option<String> {
        self.token.lock().ok().and_then(|t| t.clone())
    }

    async fn check(response: reqwest::Response) -> CmdResult<reqwest::Response> {
        if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(response);
        }
        let status = response.status();
        let message = response
            .json::<ApiErrorBody>()
            .await
            .map(|b| b.message)
            .unwrap_or_else(|_| format!("O Conexão respondeu {status}"));
        Err(CmdError::new(message))
    }

    /// Abre a sessão do Conexão: pela sessão salva da conta não original ou trocando o
    /// access token Minecraft da conta original.
    pub async fn login(&self) -> CmdResult<ConexaoUser> {
        if let Some(account) = self.offline_account() {
            let response = self
                .http
                .post(format!("{}/v1/auth/offline/session", self.base))
                .json(&serde_json::json!({ "session": account.session }))
                .send()
                .await?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(CmdError::new("Sua sessão do Discord expirou. Saia e entre de novo."));
            }
            let body: LoginResponse = Self::check(response).await?.json().await?;
            self.set_token(body.token);
            return Ok(body.user);
        }
        let credentials = default_credentials()
            .await?
            .ok_or_else(|| CmdError::new("Entre com sua conta primeiro."))?;
        let response = self
            .http
            .post(format!("{}/v1/auth/minecraft", self.base))
            .json(&serde_json::json!({ "access_token": credentials.access_token }))
            .send()
            .await?;
        let body: LoginResponse = Self::check(response).await?.json().await?;
        self.set_token(body.token);
        Ok(body.user)
    }

    async fn authed(&self, request: reqwest::RequestBuilder) -> CmdResult<reqwest::Response> {
        let token = match self.token() {
            Some(token) => token,
            None => {
                self.login().await?;
                self.token().ok_or_else(|| CmdError::new("Sessão do Conexão indisponível."))?
            }
        };
        Self::check(request.bearer_auth(token).send().await?).await
    }

    /// Quem pediu o código (IP e navegador), para o jogador conferir antes de aprovar.
    pub async fn pairing_info(&self, code: &str) -> CmdResult<PairingInfo> {
        let code: String = code.chars().filter(char::is_ascii_alphanumeric).collect();
        let request = self.http.get(format!("{}/v1/auth/pair/{code}", self.base));
        Ok(self.authed(request).await?.json().await?)
    }

    /// Token curto, só de publicação, para o `conexao-publish`.
    pub async fn publish_token(&self) -> CmdResult<String> {
        #[derive(Deserialize)]
        struct Body {
            token: String,
        }
        let request = self.http.post(format!("{}/v1/auth/publish-token", self.base));
        Ok(self.authed(request).await?.json::<Body>().await?.token)
    }

    pub async fn approve_pairing(&self, code: &str) -> CmdResult<()> {
        let request = self
            .http
            .post(format!("{}/v1/auth/pair/approve", self.base))
            .json(&serde_json::json!({ "code": code }));
        self.authed(request).await?;
        Ok(())
    }

    pub async fn me(&self) -> CmdResult<MeResponse> {
        let request = self.http.get(format!("{}/v1/me", self.base));
        Ok(self.authed(request).await?.json().await?)
    }

    /// Chamada JSON autenticada (perfis). O caminho é conferido por `api_json`.
    pub async fn json(&self, method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> CmdResult<serde_json::Value> {
        let mut request = self.http.request(method, format!("{}{path}", self.base));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = self.authed(request).await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(serde_json::Value::Null);
        }
        Ok(response.json().await?)
    }

    pub async fn upload_background(&self, bytes: Vec<u8>) -> CmdResult<serde_json::Value> {
        let request = self
            .http
            .post(format!("{}/v1/me/profile/background", self.base))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes);
        Ok(self.authed(request).await?.json().await?)
    }

    pub async fn servers(&self) -> CmdResult<Vec<ServerSummary>> {
        let response = self.http.get(format!("{}/v1/servers", self.base)).send().await?;
        Ok(Self::check(response).await?.json().await?)
    }

    pub async fn server(&self, slug: &str) -> CmdResult<ServerSummary> {
        let response = self.http.get(format!("{}/v1/servers/{slug}", self.base)).send().await?;
        Ok(Self::check(response).await?.json().await?)
    }

    pub async fn access(&self, slug: &str) -> CmdResult<AccessResponse> {
        let request = self.http.get(format!("{}/v1/me/access/{slug}", self.base));
        Ok(self.authed(request).await?.json().await?)
    }

    pub async fn apply(&self, slug: &str) -> CmdResult<AccessResponse> {
        let request = self.http.post(format!("{}/v1/servers/{slug}/whitelist/apply", self.base));
        Ok(self.authed(request).await?.json().await?)
    }

    pub async fn manifest(&self, slug: &str, etag: Option<&str>) -> CmdResult<ManifestFetch> {
        let request = |token: Option<String>| {
            let mut request = self.http.get(format!("{}/v1/servers/{slug}/manifest", self.base));
            if let Some(etag) = etag {
                request = request.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            request
        };
        // Com a lista de espera ligada, o modpack só sai para contas liberadas: vai com a sessão
        // do Conexão quando der (se o login falhar, a API explica o motivo na resposta).
        let token = match self.token() {
            Some(token) => Some(token),
            None => self.login().await.ok().and_then(|_| self.token()),
        };
        let had_token = token.is_some();
        let mut response = request(token).send().await?;
        if had_token && response.status() == reqwest::StatusCode::UNAUTHORIZED {
            // JWT vencido (dura 12 h): renova uma vez e tenta de novo.
            self.clear_token();
            let token = self.login().await.ok().and_then(|_| self.token());
            response = request(token).send().await?;
        }
        let response = Self::check(response).await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(ManifestFetch::NotModified);
        }
        let header = |name: &str| {
            response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
        };
        let etag = header(reqwest::header::ETAG.as_str());
        let signature = header(conexao_types::signing::SIGNATURE_HEADER);
        let body = response.bytes().await?;
        verify_manifest(manifest_public_key(), &body, signature.as_deref())?;
        let manifest: PackManifest = serde_json::from_slice(&body)?;
        Ok(ManifestFetch::Manifest { manifest: Box::new(manifest), etag })
    }
}

#[tauri::command]
pub async fn conexao_session(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<ConexaoUser> {
    ctx.api.login().await
}

#[tauri::command]
pub async fn my_roles(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<MyRoles> {
    let me = ctx.api.me().await?;
    Ok(MyRoles {
        is_staff: me.user.is_staff,
        servers: me
            .memberships
            .into_iter()
            .map(|m| ManagedServer { slug: m.slug, name: m.name, role: m.role })
            .collect(),
    })
}

/// Parte variável de uma rota (slug de servidor ou nick): só letras, números, `_` e `-`.
fn plain_segment(segment: &str) -> bool {
    (1..=64).contains(&segment.len()) && segment.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Rotas da API que o frontend pode chamar diretamente: perfis (leitura e escrita),
/// notícias e eventos dos servidores (só leitura), estrelas e comentários. Lista exata,
/// segmento por segmento: nada de `%2e%2e`, barras invertidas ou parâmetros que mudariam
/// o caminho real.
fn route_allowed(method: &str, path: &str) -> bool {
    if !path.starts_with("/v1/") || !path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-')) {
        return false;
    }
    let segments: Vec<&str> = path["/v1/".len()..].split('/').collect();
    if !segments.iter().all(|s| plain_segment(s)) {
        return false;
    }
    matches!(
        (method, segments.as_slice()),
        (
            "GET",
            ["me", "profile" | "stars"]
                | ["profile", "catalog"]
                | ["profiles", _]
                | ["events"]
                | ["servers", _, "posts" | "comments"]
        ) | ("PUT", ["me", "profile"] | ["servers", _, "star"])
            | ("POST", ["profiles", _, "report"] | ["servers", _, "comments"])
            | ("DELETE", ["servers", _, "star"] | ["servers", _, "comments", _])
    )
}

#[tauri::command]
pub async fn api_json(
    ctx: tauri::State<'_, Arc<AppContext>>,
    method: String,
    path: String,
    body: Option<serde_json::Value>,
) -> CmdResult<serde_json::Value> {
    if !route_allowed(&method, &path) {
        return Err(CmdError::new("rota não permitida"));
    }
    let method = match method.as_str() {
        "GET" => reqwest::Method::GET,
        "PUT" => reqwest::Method::PUT,
        "POST" => reqwest::Method::POST,
        "DELETE" => reqwest::Method::DELETE,
        _ => return Err(CmdError::new("método não permitido")),
    };
    ctx.api.json(method, &path, body).await
}

#[tauri::command]
pub async fn upload_profile_background(
    ctx: tauri::State<'_, Arc<AppContext>>,
    bytes: Vec<u8>,
) -> CmdResult<serde_json::Value> {
    if bytes.len() > 3 * 1024 * 1024 {
        return Err(CmdError::new("Imagem grande demais (máximo 3 MB)."));
    }
    ctx.api.upload_background(bytes).await
}

#[tauri::command]
pub async fn list_servers(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<Vec<ServerSummary>> {
    ctx.api.servers().await
}

#[tauri::command]
pub async fn server_access(ctx: tauri::State<'_, Arc<AppContext>>, slug: String) -> CmdResult<AccessResponse> {
    crate::deeplink::ensure_slug(&slug)?;
    ctx.api.access(&slug).await
}

#[tauri::command]
pub async fn apply_whitelist(ctx: tauri::State<'_, Arc<AppContext>>, slug: String) -> CmdResult<AccessResponse> {
    crate::deeplink::ensure_slug(&slug)?;
    ctx.api.apply(&slug).await
}

/// Token para o `conexao-publish`: vale 2 horas e só publica modpack
/// (não é a sessão do jogador, então não serve para mais nada se vazar).
#[tauri::command]
pub async fn publish_token(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<String> {
    ctx.api.publish_token().await
}

/// Mostra de onde veio o código do painel antes de o jogador aprovar.
#[tauri::command]
pub async fn pairing_info(ctx: tauri::State<'_, Arc<AppContext>>, code: String) -> CmdResult<PairingInfo> {
    ctx.api.pairing_info(&code).await
}

/// Libera o login do painel web com o código exibido nele.
#[tauri::command]
pub async fn approve_panel(ctx: tauri::State<'_, Arc<AppContext>>, code: String) -> CmdResult<()> {
    ctx.api.approve_pairing(&code).await
}

#[cfg(test)]
mod tests {
    use super::{route_allowed, verify_manifest};
    use conexao_types::signing::{public_key_b64, seed_from_bytes, sign, signing_key};

    #[test]
    fn frontend_routes_are_limited() {
        assert!(route_allowed("GET", "/v1/servers/paravexia/posts"));
        assert!(!route_allowed("POST", "/v1/servers/paravexia/posts"));
        assert!(!route_allowed("GET", "/v1/servers/paravexia/whitelist"));
        assert!(!route_allowed("GET", "/v1/servers/a/b/posts"));
        assert!(route_allowed("GET", "/v1/events"));
        assert!(route_allowed("PUT", "/v1/servers/paravexia/star"));
        assert!(route_allowed("DELETE", "/v1/servers/paravexia/star"));
        assert!(route_allowed("GET", "/v1/me/stars"));
        assert!(route_allowed("GET", "/v1/servers/paravexia/comments"));
        assert!(route_allowed("POST", "/v1/servers/paravexia/comments"));
        assert!(route_allowed("DELETE", "/v1/servers/paravexia/comments/0b9f6f0e-3f1d-4c3a-9a51-2a6d1f9c1e77"));
        assert!(!route_allowed("DELETE", "/v1/servers/paravexia"));
        assert!(!route_allowed("DELETE", "/v1/servers/paravexia/posts/0b9f6f0e-3f1d-4c3a-9a51-2a6d1f9c1e77"));
        assert!(route_allowed("PUT", "/v1/me/profile"));
        assert!(!route_allowed("GET", "/v1/profiles/../staff/moderation"));
        // Pontos codificados (%2e%2e) viram `..` no cliente HTTP: precisam ser recusados.
        assert!(!route_allowed("PUT", "/v1/profiles/%2e%2e/staff/moderation"));
        assert!(!route_allowed("POST", "/v1/profiles/%2E%2E/%2E%2E/v1/staff/moderation"));
        assert!(!route_allowed("GET", "/v1/profiles/a\\..\\staff"));
        assert!(!route_allowed("GET", "/v1/profiles/lucas?x=1"));
        assert!(!route_allowed("GET", "/v1/profiles/lucas/"));
        assert!(!route_allowed("PUT", "/v1/profiles/lucas"));
        assert!(route_allowed("GET", "/v1/profiles/Lucas_RP"));
        assert!(route_allowed("POST", "/v1/profiles/Lucas_RP/report"));
        assert!(route_allowed("GET", "/v1/profile/catalog"));
        assert!(!route_allowed("GET", "/v1/staff/moderation"));
        assert!(!route_allowed("GET", "https://evil.example/v1/events"));
    }

    #[test]
    fn manifest_signature_is_enforced_when_key_is_built_in() {
        let key = signing_key(&seed_from_bytes([3; 32])).unwrap();
        let public = public_key_b64(&key);
        let body = br#"{"version":"1.0.0"}"#;
        let signature = sign(&key, body);
        // Sem chave embutida: só aceito em build de desenvolvimento (os testes rodam em debug).
        assert_eq!(verify_manifest(None, body, None).is_ok(), cfg!(debug_assertions));
        assert!(verify_manifest(Some(&public), body, Some(&signature)).is_ok());
        assert!(verify_manifest(Some(&public), body, None).is_err());
        assert!(verify_manifest(Some(&public), b"{}", Some(&signature)).is_err());
    }
}
