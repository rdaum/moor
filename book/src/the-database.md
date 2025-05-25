# The mooR Object Database ("WorldState")

In this chapter, we describe in detail the various kinds of data that can appear in a ToastStunt database and that MOO
programs can manipulate.

The database ("WorldState") is where all of the data in a MOO is stored. Everything that players see and interact with
in a
MOO is represented in the database, including characters, objects, rooms, and the MOO programs (verbs) that give them
their
specific behaviours. If the server is restarted, it will start right back up with the same data that was present
when it was last running, allowing players to continue their interactions seamlessly.

Effectively. The database *is* the MOO.

The database is a collection of objects, each of which has a set of properties and a set of verbs. The properties are
values that can be read and written by MOO programs, while the verbs are procedures that can be called by MOO programs.
The database is organized into a hierarchy of objects, with each object having a parent object. The parent object can be
thought of as a template for the child object, providing default values for properties and default implementations for
verbs. This hierarchy allows for inheritance, where a child object can override the properties and verbs of its parent
object.