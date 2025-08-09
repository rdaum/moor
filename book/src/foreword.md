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
"moo \[in\] Rust" or "moo \[by\] Ryan" -- is an authoring system for
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

mooR is a technology layer to provide a foundation for a new kind of social media; a kind of social media that brings
back to the forefront the promise of the earlier internet, a type of interaction that is meaningful in a way the earlier
era Internet was, but designed to take advantage of the power of modern hardware and support the strengths of social
media as we know it today.

But I felt that to make that happen I couldn't just start with a fork of the original LambdaMOO (as e.g. ToastStunt had
done), but with a brand new implementation which fulfilled the following requirements:

* **Modern user experience**: Built from day 1 to meet today's expectations for rich content (images, styled text,
  video), user accessibility, and web-based interfaces that don't require custom clients. The platform and core
  database are designed with web browsers as the primary connection method.
* **Modern computing architecture**: Built to take advantage of multiple execution threads, multiple cores, and
  potentially distributed deployment across multiple servers in a datacentre environment.
* **Technological extensibility**: Built in a modular fashion to easily support new behaviors, new builtin functions,
  new protocols for connecting, new integrations to outside services, and even new languages (beyond "MOOcode") for
  writing verbs.

But why did I start from LambdaMOO -- instead of building something
new from scratch? The meaningful user experience that LambdaMOO delivered for both end-users and user-developers is core
to the goal of mooR, so LambdaMOO as the first support target serves as a good "benchmark" that preserves the foundation
of that user experience while keeping the development effort grounded and focused on concrete progress. When the system
could bring in and successfully run an existing LambdaMOO core database -- and support development of further features
beyond that -- then it would be ready for release.

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
ColdMUD -- ideas from those systems have made their way into mooR as
well.

I hope you, the reader, enjoy the system we've put together. Even more
so I welcome your contributions and suggestions. If you find value in
mooR and would like to support its ongoing development, please consider
[sponsoring the project](https://github.com/sponsors/rdaum).

Ryan Daum (written on an airplane flying over the Canadian prairies,
Feb 5, 2025)
