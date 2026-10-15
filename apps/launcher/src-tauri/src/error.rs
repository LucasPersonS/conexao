use serde::Serialize;

/// Erro enviado ao frontend como `{ message }`.
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub message: String,
}

pub type CmdResult<T> = Result<T, CmdError>;

impl CmdError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl From<theseus::Error> for CmdError {
    fn from(error: theseus::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<reqwest::Error> for CmdError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_connect() || error.is_timeout() {
            Self::new("Não foi possível falar com o Conexão. Verifique sua conexão.")
        } else {
            Self::new(format!("Erro de rede: {error}"))
        }
    }
}

impl From<std::io::Error> for CmdError {
    fn from(error: std::io::Error) -> Self {
        Self::new(format!("Erro de arquivo: {error}"))
    }
}

impl From<tauri::Error> for CmdError {
    fn from(error: tauri::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<zip::result::ZipError> for CmdError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::new(format!("Erro ao montar o modpack: {error}"))
    }
}

impl From<serde_json::Error> for CmdError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(format!("Resposta inválida: {error}"))
    }
}
