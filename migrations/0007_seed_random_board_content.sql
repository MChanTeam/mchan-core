INSERT INTO threads(board_id, title, body, status, poster_id)
SELECT
  id,
  'What small win are you taking into next week?',
  'A small, repeatable win can matter more than a dramatic breakthrough. What did you finish, learn, or improve this week that you want to carry forward?',
  'visible',
  'Anonymous'
FROM boards
WHERE slug = 'b';

INSERT INTO threads(board_id, title, body, status, poster_id)
SELECT
  id,
  'How do you build a useful professional network?',
  'I am trying to approach networking as a long-term practice rather than a numbers game. Which habits have helped you build genuine professional relationships?',
  'visible',
  'Anonymous'
FROM boards
WHERE slug = 'b';

INSERT INTO threads(board_id, title, body, status, poster_id)
SELECT
  id,
  'Learning in public without turning it into a performance',
  'Sharing progress can create accountability and invite helpful feedback. How do you keep public learning honest, useful, and sustainable?',
  'visible',
  'Anonymous'
FROM boards
WHERE slug = 'b';

INSERT INTO threads(board_id, title, body, status, poster_id)
SELECT
  id,
  'What career lesson took you the longest to learn?',
  'The lessons that stay with us are often about communication, boundaries, or patience rather than a particular tool. What would you tell your earlier self?',
  'visible',
  'Anonymous'
FROM boards
WHERE slug = 'b';

INSERT INTO threads(board_id, title, body, status, poster_id)
SELECT
  id,
  'A thoughtful way to ask for feedback',
  'Specific questions make feedback easier to give and easier to act on. What prompts have helped you get feedback that is concrete, kind, and useful?',
  'visible',
  'Anonymous'
FROM boards
WHERE slug = 'b';

INSERT INTO threads(board_id, title, body, status, poster_id)
VALUES
  (
    (SELECT id FROM boards WHERE slug = 'b'),
    'What is a work habit you quietly recommend?',
    'Not every useful practice needs to be trendy. What simple habit has made your work calmer, clearer, or more consistent?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'b'),
    'How do you make introductions more helpful?',
    'A good introduction gives people enough context to start a real conversation. What details make an introduction feel natural rather than transactional?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'b'),
    'What makes a meeting worth attending?',
    'Clear purpose, thoughtful preparation, and a useful next step can make a big difference. What is one meeting practice you wish more teams used?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'b'),
    'How do you protect time for deep work?',
    'Notifications and busy calendars can make focused work difficult. Which boundaries or routines help you create space for work that needs sustained attention?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'b'),
    'What professional topic are you curious about right now?',
    'Curiosity is a good reason to start a conversation. What are you exploring, and what would you like to understand better from people with experience?',
    'visible',
    'Anonymous'
  );

INSERT INTO threads(board_id, title, body, status, poster_id)
VALUES
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What makes a code review genuinely useful?',
    'A strong review improves the result without turning into a style contest. Which review habits help your team discuss correctness, maintainability, and trade-offs?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'How do you explain technical trade-offs to non-engineers?',
    'The clearest explanation usually starts with the decision and its impact. What approaches help you make technical constraints understandable to partners and stakeholders?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What did a production incident teach you?',
    'Incidents can expose gaps in systems and communication. What lesson changed how you design, test, document, or support software?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'How do you decide when to refactor?',
    'Refactoring is a balance between reducing future cost and delivering current value. What signals help you choose the right time to improve an existing design?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What makes documentation stay useful?',
    'Documentation works best when it answers a real question close to where that question arises. Which lightweight practices keep technical docs accurate and discoverable?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'How do you help a teammate ramp up?',
    'Good onboarding is more than sharing a list of links. What has helped new teammates build context, confidence, and a useful first contribution?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What is one testing lesson you learned the hard way?',
    'Tests are most valuable when they protect behavior that matters. Which testing lesson changed how you choose cases, fixtures, or failure modes?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'How do you keep projects moving through uncertainty?',
    'Requirements and constraints change. What communication or planning habits help your team make progress without pretending that unknowns do not exist?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What small automation saved you time?',
    'A small script or workflow can remove repeated friction. What did you automate, and what did it teach you about improving everyday engineering work?',
    'visible',
    'Anonymous'
  ),
  (
    (SELECT id FROM boards WHERE slug = 'engineering'),
    'What advice would you give an early-career engineer?',
    'Tools change quickly, but some habits compound for years. What would you tell someone building their technical judgment and professional confidence?',
    'visible',
    'Anonymous'
  );

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'I finally finished a task I had been postponing and wrote down the next step before closing the laptop. The follow-through felt better than the size of the task.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'What small win are you taking into next week?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'I have found that consistency beats a polished announcement. A short update with what changed, what I learned, and what I will try next is enough.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'What small win are you taking into next week?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'Following up after a conversation has helped most: mention something specific from the discussion and offer a useful resource without immediately asking for anything.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'How do you build a useful professional network?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'I try to be curious before being strategic. People can usually tell when the goal is to understand their work rather than collect another contact.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'How do you build a useful professional network?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'The most useful posts I read include a real constraint or mistake. That gives others something concrete to learn from instead of presenting a perfect process.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'Learning in public without turning it into a performance';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'Setting a small publishing rhythm helps. If there is no new insight, I would rather wait than turn an ordinary day into an exaggerated success story.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'Learning in public without turning it into a performance';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'Clear writing is a career skill. I spent too long assuming that good work would speak for itself; explaining the decision and its trade-offs makes collaboration much easier.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'What career lesson took you the longest to learn?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'You do not need to have the whole next chapter planned before changing direction. A small experiment can provide better evidence than months of speculation.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'What career lesson took you the longest to learn?';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'I ask, “What is one thing I could make clearer or more effective?” It invites a specific answer and signals that I am ready to do something with it.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'A thoughtful way to ask for feedback';

INSERT INTO replies(thread_id, body, status, poster_id)
SELECT
  t.id,
  'It also helps to say what kind of feedback you need: the idea, the structure, the communication, or the execution. Broad requests often produce broad answers.',
  'visible',
  'Anonymous'
FROM threads AS t
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b'
  AND t.title = 'A thoughtful way to ask for feedback';
WITH seeded_replies(title, body) AS (
  VALUES
    (
      'What is a work habit you quietly recommend?',
      'I write down the next action before ending a work session. It reduces the energy needed to restart and keeps small commitments visible.'
    ),
    (
      'How do you make introductions more helpful?',
      'A sentence about why the two people might enjoy speaking is more useful than a list of job titles. It gives the conversation a thoughtful starting point.'
    ),
    (
      'What makes a meeting worth attending?',
      'A short written decision and clear owners at the end make a meeting valuable. If neither is needed, an update or document may be a better format.'
    ),
    (
      'How do you protect time for deep work?',
      'I reserve a few recurring blocks and make the expected output explicit. A visible boundary is easier for teammates to respect than an informal hope for focus.'
    ),
    (
      'What professional topic are you curious about right now?',
      'I am curious about how teams preserve good judgment as they grow. Processes help, but the examples leaders reward often shape culture even more.'
    )
)
INSERT INTO replies(thread_id, body, status, poster_id)
SELECT t.id, s.body, 'visible', 'Anonymous'
FROM seeded_replies AS s
JOIN threads AS t ON t.title = s.title
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'b';

WITH seeded_replies(title, body) AS (
  VALUES
    (
      'What makes a code review genuinely useful?',
      'I appreciate reviews that separate correctness issues from optional suggestions. That makes the important feedback clear without discouraging discussion.'
    ),
    (
      'How do you explain technical trade-offs to non-engineers?',
      'I compare options by user impact, cost, and reversibility. Those dimensions usually make the decision easier to discuss than implementation details alone.'
    ),
    (
      'What did a production incident teach you?',
      'The best incident reviews focus on system conditions rather than blame. We improved the outcome most when we turned each lesson into a small owned action.'
    ),
    (
      'How do you decide when to refactor?',
      'Repeated changes in the same area are a useful signal. If each change takes longer or creates more risk, a focused refactor can be part of responsible delivery.'
    ),
    (
      'What makes documentation stay useful?',
      'An owner, a last-reviewed date, and examples that can be checked quickly make documentation easier to trust. Short and current beats comprehensive and stale.'
    ),
    (
      'How do you help a teammate ramp up?',
      'I pair on one small real task, explain how to find answers, and schedule a follow-up. That builds context while giving the new teammate an early sense of progress.'
    ),
    (
      'What is one testing lesson you learned the hard way?',
      'A test that only checks a happy path can create false confidence. Boundary cases and meaningful failure behavior deserve attention early, not after the incident.'
    ),
    (
      'How do you keep projects moving through uncertainty?',
      'Making the next decision explicit helps. We can acknowledge what is unknown while still agreeing on the smallest useful experiment and when we will revisit it.'
    ),
    (
      'What small automation saved you time?',
      'A command that prepares a consistent local environment removed several minutes of setup from every task. The bigger lesson was to notice repeated friction before accepting it.'
    ),
    (
      'What advice would you give an early-career engineer?',
      'Learn to explain why a decision was made, not only how to implement it. Technical judgment grows through context, feedback, and seeing the consequences over time.'
    )
)
INSERT INTO replies(thread_id, body, status, poster_id)
SELECT t.id, s.body, 'visible', 'Anonymous'
FROM seeded_replies AS s
JOIN threads AS t ON t.title = s.title
JOIN boards AS b ON b.id = t.board_id
WHERE b.slug = 'engineering';
