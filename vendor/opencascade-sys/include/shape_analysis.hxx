#include <ShapeAnalysis.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <bindings_common.hxx>

struct RustShapeAnalysis_FreeBounds {
	static void ConnectEdgesToWires(Handle_TopTools_HSequenceOfShape &edges, double tolerance,
																	bool shared, Handle_TopTools_HSequenceOfShape &wires) {
		ShapeAnalysis_FreeBounds::ConnectEdgesToWires(edges, tolerance, shared, wires);
	}
};
