#include <BRepLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <TopoDS_Shape.hxx>
#include <bindings_common.hxx>

struct RustBRepLib_ToolTriangulatedShape {
	static void ComputeNormals(const TopoDS_Face &face, const Handle_Poly_Triangulation &triangulation) {
		if (triangulation->NbNodes() == 0) return;
		BRepLib_ToolTriangulatedShape::ComputeNormals(face, triangulation);
	}
};
