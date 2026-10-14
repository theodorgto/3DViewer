#include <BRepMesh_IncrementalMesh.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <b_rep.hxx>
#include <bindings_common.hxx>
#include <cstdint>
#include <limits>
#include <memory>
#include <stdexcept>

inline std::unique_ptr<BRepMesh_IncrementalMesh> viewer_incremental_mesh(
		const TopoDS_Shape &shape, const double deflection) {
	return std::unique_ptr<BRepMesh_IncrementalMesh>(
			new BRepMesh_IncrementalMesh(shape, deflection, false, 0.5, true));
}

struct RenderMesh {
	rust::Vec<double> points;
	rust::Vec<std::uint32_t> triangles;

	rust::Slice<const double> positions() const { return {points.data(), points.size()}; }
	rust::Slice<const std::uint32_t> indices() const { return {triangles.data(), triangles.size()}; }
};

inline std::unique_ptr<RenderMesh> render_mesh(const TopoDS_Shape &shape, const double deflection) {
	const auto mesher = viewer_incremental_mesh(shape, deflection);
	if (!mesher->IsDone()) throw std::runtime_error("Cannot tessellate STEP surfaces");
	std::unique_ptr<RenderMesh> mesh(new RenderMesh());
	for (TopExp_Explorer explorer(mesher->Shape(), TopAbs_FACE); explorer.More(); explorer.Next()) {
		const TopoDS_Face face = TopoDS::Face(explorer.Current());
		TopLoc_Location location;
		const auto handle = BRep_Tool_Triangulation(face, location);
		const Handle_Poly_Triangulation &triangulation = *handle;
		const std::size_t offset = mesh->points.size() / 3;
		const auto node_count = triangulation->NbNodes();
		const auto triangle_count = triangulation->NbTriangles();
		const auto limit = static_cast<std::size_t>(std::numeric_limits<std::int32_t>::max());
		if (offset + node_count > limit || mesh->triangles.size() + std::size_t(triangle_count) * 3 > limit) {
			throw std::runtime_error("Model exceeds renderer limits");
		}
		for (Standard_Integer node = 1; node <= node_count; ++node) {
			gp_Pnt point = triangulation->Node(node);
			point.Transform(location.Transformation());
			mesh->points.push_back(point.X());
			mesh->points.push_back(point.Y());
			mesh->points.push_back(point.Z());
		}
		const bool forward = face.Orientation() == TopAbs_FORWARD;
		for (Standard_Integer index = 1; index <= triangle_count; ++index) {
			const Poly_Triangle &triangle = triangulation->Triangle(index);
			for (Standard_Integer corner = 1; corner <= 3; ++corner) {
				const auto node = triangle.Value(forward ? corner : 4 - corner);
				if (node < 1 || node > node_count) throw std::runtime_error("Invalid STEP triangle index");
				mesh->triangles.push_back(static_cast<std::uint32_t>(offset + node - 1));
			}
		}
	}
	return mesh;
}
