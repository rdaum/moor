object AGENTIC_EVENT_QUEUE [
  import_export_id -> "agentic_event_queue",
  import_export_hierarchy -> {"agentic"}
]
  name: "Agentic Event Queue"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property queue (owner: ARCH_WIZARD, flags: "rc") = {};

  override description (owner: ARCH_WIZARD, flags: "rc") = "Queue helper for agentic runners and observers.";

  method push owner: ARCH_WIZARD
    "Append an item with the caller's permissions. Return the new queue length.";
    set_task_perms(caller_perms());
    const {item} = args;
    this.queue = {@this.queue, item};
    return length(this.queue);
  endmethod

  method pop owner: ARCH_WIZARD
    "Remove the first item with the caller's permissions. Return 0 when empty.";
    set_task_perms(caller_perms());
    length(this.queue) == 0 && return 0;
    const item = this.queue[1];
    this.queue = length(this.queue) > 1 ? this.queue[2..$] | {};
    return item;
  endmethod

  method size owner: ARCH_WIZARD
    "Return the queue length with the caller's permissions.";
    set_task_perms(caller_perms());
    return length(this.queue);
  endmethod

  method clear owner: ARCH_WIZARD
    "Clear the queue with the caller's permissions. Return 1.";
    set_task_perms(caller_perms());
    this.queue = {};
    return 1;
  endmethod
endobject
