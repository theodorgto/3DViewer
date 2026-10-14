pub use inner::*;

#[cxx::bridge]
mod inner {
    unsafe extern "C++" {
        include!("opencascade-sys/include/b_rep_mesh.hxx");

        type TopoDS_Shape = crate::topo_ds::TopoDS_Shape;

        type BRepMesh_IncrementalMesh;
        #[cxx_name = "viewer_incremental_mesh"]
        fn IncrementalMesh_new(
            shape: &TopoDS_Shape,
            deflection: f64,
        ) -> UniquePtr<BRepMesh_IncrementalMesh>;
        fn Shape(self: &BRepMesh_IncrementalMesh) -> &TopoDS_Shape;
        fn IsDone(self: &BRepMesh_IncrementalMesh) -> bool;

        type RenderMesh;
        pub fn render_mesh(shape: &TopoDS_Shape, deflection: f64) -> Result<UniquePtr<RenderMesh>>;
        pub fn positions(self: &RenderMesh) -> &[f64];
        pub fn indices(self: &RenderMesh) -> &[u32];
    }
}
