#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <GProp_GProps.hxx>
#include <Poly_Triangulation.hxx>
#include <cmath>
#include <atomic>
#include <iostream>
#include <Geom_Curve.hxx>
#include <Geom_Surface.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS_Builder.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <bindings_common.hxx>
#include <gp_Pnt.hxx>

inline const TopoDS_Builder &BRep_Builder_upcast_to_topods_builder(const BRep_Builder &builder) { return builder; }

inline std::atomic<std::size_t> &cad_skipped_face_counter() {
  static std::atomic<std::size_t> count{0};
  return count;
}

inline std::size_t skipped_cad_faces() { return cad_skipped_face_counter().load(); }
inline void reset_skipped_cad_faces() { cad_skipped_face_counter().store(0); }

inline std::unique_ptr<Handle_Geom_Surface> BRep_Tool_Surface(const TopoDS_Face &face) {
  return std::unique_ptr<Handle_Geom_Surface>(new Handle_Geom_Surface(BRep_Tool::Surface(face)));
}

inline std::unique_ptr<Handle_Geom_Curve> BRep_Tool_Curve(const TopoDS_Edge &edge, Standard_Real &first,
                                                          Standard_Real &last) {
  return std::unique_ptr<Handle_Geom_Curve>(new Handle_Geom_Curve(BRep_Tool::Curve(edge, first, last)));
}

inline std::unique_ptr<gp_Pnt> BRep_Tool_Pnt(const TopoDS_Vertex &vertex) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(BRep_Tool::Pnt(vertex)));
}

inline std::unique_ptr<Handle_Poly_Triangulation> BRep_Tool_Triangulation(const TopoDS_Face &face,
                                                                          TopLoc_Location &location) {
  Handle_Poly_Triangulation triangulation = BRep_Tool::Triangulation(face, location);
  if (triangulation.IsNull()) {
    GProp_GProps properties;
    BRepGProp::SurfaceProperties(face, properties);
    if (std::abs(properties.Mass()) <= 1e-10) {
      triangulation = new Poly_Triangulation();
    } else {
      BRepMesh_IncrementalMesh retry(face, 0.001, false, 0.2, false);
      triangulation = BRep_Tool::Triangulation(face, location);
      if (triangulation.IsNull()) {
        std::cerr << "STEP: untriangulated face area " << properties.Mass() << std::endl;
        cad_skipped_face_counter().fetch_add(1);
        triangulation = new Poly_Triangulation();
      }
    }
  }
  return std::unique_ptr<Handle_Poly_Triangulation>(
      new Handle_Poly_Triangulation(triangulation));
}
