object GENERIC_UTILS
  name: "Generic Utilities Package"
  parent: ROOT_CLASS
  owner: #2
  fertile: true
  readable: true

  property help_msg (owner: #2, flags: "rc") = {
    "This is the Generic Utility Object.  One presumes it should have text in it explaining the use of the utility object in question."
  };

  override aliases = {"Generic Utilities Package"};
  override description = "This is a placeholder parent for all the $..._utils packages, to more easily find them and manipulate them. At present this object defines no useful verbs or properties. (Filfre.)";
  override import_export_id = "generic_utils";
  override object_size = {579, 1084848672};
endobject