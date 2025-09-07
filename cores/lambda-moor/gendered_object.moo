object GENDERED_OBJECT
  name: "Generic Gendered Object"
  parent: ROOT_CLASS
  owner: BYTE_QUOTA_UTILS_WORKING
  fertile: true
  readable: true

  property gender (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "neuter";
  property po (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "it";
  property poc (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "It";
  property pp (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "its";
  property ppc (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "Its";
  property pq (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "its";
  property pqc (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "its";
  property pr (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "itself";
  property prc (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "Itself";
  property ps (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "it";
  property psc (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "It";

  override aliases = {"Generic Gendered Object"};
  override object_size = {2378, 1084848672};

  verb set_gender (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "set_gender(newgender) attempts to change this.gender to newgender";
    "  => E_PERM   if you don't own this or aren't its parent";
    "  => Other return values as from $gender_utils:set.";
    if (!($perm_utils:controls(caller_perms(), this) || this == caller))
      return E_PERM;
    else
      result = $gender_utils:set(this, args[1]);
      this.gender = typeof(result) == STR ? result | args[1];
      return result;
    endif
  endverb

  verb "@gen*der" (this is any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (player.wizard || player == this.owner)
      player:tell(this:set_gender(iobjstr) ? "Gender and pronouns set." | "Gender set.");
    else
      player:tell("Permission denied.");
    endif
  endverb

  verb verb_sub (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Copied from generic player (#6):verb_sub by ur-Rog (#6349) Fri Jan 22 11:20:11 1999 PST";
    "This verb was copied by TheCat on 01/22/99, so that the generic gendered object will be able to do verb conjugation as well as pronoun substitution.";
    text = args[1];
    if (a = `$list_utils:assoc(text, this.verb_subs) ! ANY')
      return a[2];
    else
      return $gender_utils:get_conj(text, this);
    endif
  endverb
endobject