#include <BRepOffsetAPI_MakeOffset.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <Law_Function.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS_Shape.hxx>
#include <bindings_common.hxx>

class RustBRepOffsetAPI_MakePipeShell : public BRepOffsetAPI_MakePipeShell {
public:
	using BRepOffsetAPI_MakePipeShell::BRepOffsetAPI_MakePipeShell;
	void SetLaw(const TopoDS_Shape &profile, const Handle_Law_Function &law,
							bool with_contact, bool with_correction) {
		BRepOffsetAPI_MakePipeShell::SetLaw(profile, law, with_contact, with_correction);
	}
};
