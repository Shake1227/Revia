use std::{
    io,
    path::{Path, PathBuf},
};

pub trait Trash: Send + Sync {
    fn move_to_trash(&self, path: &Path) -> io::Result<()>;
}
pub struct OsTrash;
impl Trash for OsTrash {
    fn move_to_trash(&self, path: &Path) -> io::Result<()> {
        let context = ::trash::TrashContext::default();
        #[cfg(target_os = "macos")]
        let context = {
            use ::trash::macos::{DeleteMethod, TrashContextExtMacos};
            let mut context = context;
            context.set_delete_method(DeleteMethod::NsFileManager);
            context
        };
        context
            .delete(path)
            .map_err(|e| io::Error::other(format!("Cannot move item to trash: {e}")))
    }
}
pub struct MoveTo(pub PathBuf);
impl Trash for MoveTo {
    fn move_to_trash(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(&self.0)?;
        let destination = self.0.join(uuid::Uuid::new_v4().to_string());
        std::fs::rename(path, destination)
    }
}
