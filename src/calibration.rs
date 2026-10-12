use crate::mesh::WarpMesh;

#[derive(Default)]
pub struct MeshHistory {
    undo: Vec<Option<WarpMesh>>,
    redo: Vec<Option<WarpMesh>>,
    saved: Option<WarpMesh>,
    editing: Option<Option<WarpMesh>>,
}
impl MeshHistory {
    pub fn new(mesh: &Option<WarpMesh>) -> Self {
        Self {
            saved: mesh.clone(),
            ..Self::default()
        }
    }
    pub fn validate_candidate(mesh: &WarpMesh) -> anyhow::Result<()> {
        mesh.validate()?;
        anyhow::ensure!(!mesh.triangles().is_empty(), "No drawable mesh cells reach the dome. Adjust the geometry; the current calibration has been kept.");
        Ok(())
    }
    fn push(&mut self, mesh: Option<WarpMesh>) {
        if self.undo.len() == 20 {
            self.undo.remove(0);
        }
        self.undo.push(mesh);
        self.redo.clear();
    }
    pub fn apply(&mut self, current: &mut Option<WarpMesh>, next: WarpMesh) -> anyhow::Result<()> {
        Self::validate_candidate(&next)?;
        self.end_edit(current);
        self.push(current.clone());
        *current = Some(next);
        Ok(())
    }
    pub fn begin_edit(&mut self, current: &Option<WarpMesh>) {
        if self.editing.is_none() {
            self.editing = Some(current.clone());
        }
    }
    pub fn end_edit(&mut self, current: &Option<WarpMesh>) {
        if let Some(before) = self.editing.take() {
            if &before != current {
                self.push(before);
            }
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self, current: &mut Option<WarpMesh>) {
        self.end_edit(current);
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(current, previous));
        }
    }
    pub fn redo(&mut self, current: &mut Option<WarpMesh>) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(current, next));
        }
    }
    pub fn mark_saved(&mut self, current: &Option<WarpMesh>) {
        self.saved = current.clone();
    }
    pub fn modified(&self, current: &Option<WarpMesh>) -> bool {
        &self.saved != current
    }
}
