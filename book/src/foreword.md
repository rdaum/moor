# Foreword

Welcome to the manual for mooR. 

First, credit where credit is due: The documentation provided here is
a product of the hard work of many authors over multiple decades,
beginning with the original LambdaMOO manual from the early 90s,
through to the documentation written by Brendan ("aka Slither") for
ToastStunt, in 2022. We've made the earnest effort to credit those
authors throughout, but we'll state up front now that if there's a
place where that has been missed, we urge you to let us know, and
we'll make sure to provide it.

Secondly, some basic background and definitions. I'll be brief here
because the [introduction](./introduction.md) section gets into more
detail, but:

*mooR* -- which stands for "moo-reconstructed" or "moo - rewrite" or
"moo [in] Rust" or "moo [by] Ryan" -- is an authoring system for
multiuser / multiplayer online communities. It is both a fully
compatible rewrite of LambdaMOO -- a pioneering super-flexible
object-oriented "MUD" server from the 1990s -- and a modernized and
flexible platform on which to build dynamic, fun, multiuser/multipler
connected communities and games.

Thirdly, motivations and reasons.

This is a project I began on a _"it can't be that hard"_ whim in the
fall of 2022, with the intent of trying to revive the ideas behind MOO
but with a more "21st century" technological foundation that would
make it easier to maintain and scale such applications going forward.

I did this because I perceived then (and still do) that there is a
problem in the way the "social _media_" landscape has evolved, and
felt the desire to see a return of an earlier type of interaction on
the Internet. But felt that to make that happen I couldn't just start
with a fork of the original LambdaMOO (as e.g. ToastStunt had done),
but with a brand new implementation which fulfilled the following
requirements:

  * That it be built from day 1 to be able to meet the expectations of
    today's users to provide "rich" content (images, styled text,
    video) and not require a custom client. So to start with the idea
    that the user would be connecting by a web browser, and to make
    the platform and core database with that in mind.
  * That it be built from day 1 with modern computers in mind -- to
    take advantage of multiple threads, on multiple cores, potentially
    distributed across multiple machines in a datacentre.
  * That it be built in such a way that it would be easier to extend
    and add behaviours -- new builtin functions, new protocols for
    connecting, new integrations to outside services, and even new
    languages (beyond "MOOcode") for writing verbs. And so be built in
    a modular fashion.

But why did I start from LambdaMOO -- instead of building something
new from scratch? Nostalgia could be one explanation, but the primary
motivator was the desire to have a "benchmark" to measure my
deliverables. When the system could bring in and successfully run an
existing LambdaMOO core database -- and offer additional features on
top of that -- then it would be ready for release.

The choice of Rust as the implementation language for mooR was driven
by many reasons, which I need not go into here. But I feel that it is
has overall served the project well, and allowed me to develop with
confidence.

It is now the winter of 2025, and the project that I began over two
years ago is circling around to its first major, public, 1.0
release. Many hundreds of hours have gone into development -- not just
by myself, but by others who have put immense effort into developing and
testing and suggesting. Thanks goes not just to them, but to the
original authors and users of LambdaMOO, Stunt, ToastStunt, and to the
adjacent projects and communities I (and others) was a part of over
the years, in particular Stephen White's CoolMUD and Greg Hudson's
ColdMUD -- ideas from those sytems have made their way into mooR as
well.

I hope you, the reader, enjoy the system we've put together. Even more
so I welcome your contributions and suggestions.

Ryan Daum (written on an airplane flying over the Canadian prairies,
Feb 5, 2025)
