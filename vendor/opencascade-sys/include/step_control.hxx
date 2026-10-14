#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <ShapeFix_Shape.hxx>
#include <bindings_common.hxx>
#include <chrono>
#include <cstdlib>
#include <iostream>

inline void trace_step_phase(const char *phase, std::chrono::steady_clock::time_point started) {
  if (std::getenv("MODEL_VIEWER_IMPORT_TRACE")) {
    const double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - started).count();
    std::cerr << "STEP: " << phase << " in " << seconds << "s" << std::endl;
  }
}

inline IFSelect_ReturnStatus read_step(STEPControl_Reader &reader, rust::String theFileName) {
  const auto started = std::chrono::steady_clock::now();
  const auto status = reader.ReadFile(theFileName.c_str());
  trace_step_phase("parsed file", started);
  return status;
}

inline Standard_Integer transfer_step_roots(STEPControl_Reader &reader, const Message_ProgressRange &progress) {
  const auto started = std::chrono::steady_clock::now();
  const auto count = reader.TransferRoots(progress);
  trace_step_phase("transferred roots", started);
  return count;
}

inline std::unique_ptr<TopoDS_Shape> one_shape_step(const STEPControl_Reader &reader) {
  const auto started = std::chrono::steady_clock::now();
  ShapeFix_Shape healing(reader.OneShape());
  healing.Perform();
  trace_step_phase("healed shape", started);
  return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(healing.Shape()));
}

inline IFSelect_ReturnStatus transfer_shape(STEPControl_Writer &writer, const TopoDS_Shape &theShape) {
  return writer.Transfer(theShape, STEPControl_AsIs);
}

inline IFSelect_ReturnStatus write_step(STEPControl_Writer &writer, rust::String theFileName) {
  return writer.Write(theFileName.c_str());
}
