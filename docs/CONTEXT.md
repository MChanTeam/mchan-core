# MChan

MChan is an anonymous imageboard where public posts may be screened before publication and reviewed by human moderators afterward.

## Language

**Text Screening**:
An automated, pre-publication assessment of thread or reply text by Miya. It informs publication and moderator attention but is not a moderator action.
_Avoid_: Automated moderation, AI moderation

**Flagged Post**:
A published thread or reply placed in the moderation queue because Text Screening returned `review`. It remains public unless a moderator changes its status.
_Avoid_: Pending post, held submission

**Screening Audit**:
A metadata record of a Text Screening result or outage that excludes the post text. An outage audit does not by itself place the post in the moderation queue.
_Avoid_: Miya log, content log

**Blocked Submission**:
A thread or reply submission rejected before publication because Text Screening returned `block`. It is not a post and does not create a ban or moderator action.
_Avoid_: Blocked post, automatic ban

**Screening Flag**:
Automated screening evidence attached to a published post when Text Screening returned `review`. It appears in the moderation queue but is distinct from a visitor-submitted Report.
_Avoid_: Automated report, system report

**Media Processing**:
The stateless transformation of an untrusted image attachment into safe display and thumbnail variants plus metadata. It does not own durable media storage or post lifecycle.
_Avoid_: Image storage, upload storage

**Post Submission**:
A proposed thread or reply before publication. It becomes a public post only after required screening, optional Media Processing, and persistence succeed.
_Avoid_: Post, persisted post
