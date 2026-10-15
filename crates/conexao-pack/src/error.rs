#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// Erro de validação com mensagem pronta para o administrador.
    #[error("{0}")]
    Invalid(String),
    #[error("erro de leitura em {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("arquivo ZIP/JAR inválido: {0}")]
    Zip(#[from] zip::result::ZipError),
}

pub(crate) fn fail<T>(message: impl Into<String>) -> crate::Result<T> {
    Err(PackError::Invalid(message.into()))
}

pub(crate) fn io_err(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> PackError + '_ {
    move |source| PackError::Io { path: path.display().to_string(), source }
}
