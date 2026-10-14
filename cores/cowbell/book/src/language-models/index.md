# Characters and tools that use language models

Cowbell includes optional characters and tools that accept requests in ordinary language. Mr.
Welcome can answer questions about the world. The Architect's Compass provides building tools. The
Data Visor provides tools for exploring code and objects.

A **language model** produces responses to supplied text. An **agent** combines a model with tools
that can inspect or act on the world. Each tool supplies a particular operation, such as finding an
object or creating a room. A model's request still passes through the tool's permission checks.

These facilities need model access and configuration. Their presence in the starting core does not
mean they are ready to use on every server. The configuration walkthrough still needs to be written.

## Three guides are needed

**Using the supplied characters and tools.** This guide will walk through a request, the response,
any confirmation, and the result in the world. It must cover stopping a request and understanding
failures, with the available interfaces in Meadow and a terminal.

**Connecting and operating a model.** This guide will connect server workers, model credentials, and
object settings in one procedure. It must explain whose credentials pay for a request, what
information leaves the world, and how to inspect usage.

**Writing agents and tools.** This guide will show how to give an agent a small, specific task. It
must explain observations, tool inputs, permissions, cancellation, and how to check the result. The
source contains more than one agent implementation; the guide must establish which path to use
before recommending one to authors.

These are planned chapters. For a character you can build today without model configuration, follow
[Henri's tutorial](../actors/henri-and-the-cupboard.md).
