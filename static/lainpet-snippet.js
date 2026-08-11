(() => {
  "use strict";

  const SPRITE_SIZE = 64;
  const EXPRESSION_SIZE = 36;
  const BUBBLE_WIDTH = 150;

  const BUBBLE_GAP = 46;
  const EXPRESSION_GAP = 40;

  const WALK_SPEED = 3;
  const FRAME_DURATION = 30;
  const MAX_FRAME_ELAPSED = 100;

  const IDLE_DURATION = 5000;
  const MOVEMENT_TIMEOUT = 10000;
  const DIALOGUE_DURATION = 4000;
  const EXPRESSION_DURATION = 3000;

  const WALK_MIN_DISTANCE = 100;
  const WALK_MAX_DISTANCE = 1000;
  const TARGET_RADIUS = 5;
  const TARGET_TOLERANCE = 1e-6;

  const WAVE_AMPLITUDE = 40;
  const WAVE_CYCLES = 1.5;
  const PARABOLA_HEIGHT = 80;

  const WALK_CHANCE_PER_FRAME = 0.005;
  const SINE_PATH_CHANCE = 0.15;
  const PARABOLA_PATH_CHANCE = 0.15;

  const AMBIENT_INTERVAL = 15000;
  const OUTFIT_INTERVAL = 60000;
  const DIALOGUE_CHANCE = 0.2;
  const EXPRESSION_CHANCE = 0.2;

  const assets = {
    default: {
      idle: "/static/lainpet-assets/1.png",
      right: "/static/lainpet-assets/lainwalk1.gif",
      left: "/static/lainpet-assets/lainwalk2.gif"
    },
    school: {
      idle: "/static/lainpet-assets/115.png",
      right: "/static/lainpet-assets/lainwalk3.gif",
      left: "/static/lainpet-assets/lainwalk4.gif"
    },
    pink: {
      idle: "/static/lainpet-assets/116.png",
      right: "/static/lainpet-assets/lainwalk5.gif",
      left: "/static/lainpet-assets/lainwalk6.gif"
    },
    bear: {
      idle: "/static/lainpet-assets/117.png",
      right: "/static/lainpet-assets/lainwalk7.gif",
      left: "/static/lainpet-assets/lainwalk8.gif"
    },
    home: {
      idle: "/static/lainpet-assets/118.png",
      right: "/static/lainpet-assets/lainwalk9.gif",
      left: "/static/lainpet-assets/lainwalk10.gif"
    },
    expressions: {
      default: "/static/lainpet-assets/expression1.gif",
      bear: "/static/lainpet-assets/expression2.gif"
    }
  };

  const OUTFITS = ["default", "school", "pink", "bear", "home"];

  const dialogues = [
    "Present day. Present time. Hahahaha!",
    "Why don't you come to the Wired?",
    "No matter where you are, everyone is always connected.",
    "You're wrong.",
    "Don't worry. I'm still me.",
    "check out mxrza.xyz",
    "Let's All Love Lain.",
    "Why are you crying Lain",
    "The real world isn't real at all.",
    "also check out kuumin (website pending)"
  ];

  function addVectors(a, b) {
    return { x: a.x + b.x, y: a.y + b.y };
  }

  function subtractVectors(a, b) {
    return { x: a.x - b.x, y: a.y - b.y };
  }

  function scaleVector(vector, scalar) {
    return { x: vector.x * scalar, y: vector.y * scalar };
  }

  function vectorLength(vector) {
    return Math.hypot(vector.x, vector.y);
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(value, max));
  }

  function rotateCurve(curve, degrees) {
    const radians = (degrees * Math.PI) / 180;
    const cosine = Math.cos(radians);
    const sine = Math.sin(radians);

    return (progress) => {
      const point = curve(progress);
      return {
        x: point.x * cosine - point.y * sine,
        y: point.x * sine + point.y * cosine
      };
    };
  }

  function sineCurve(amplitude, cycles) {
    return (progress) => ({
      x: progress,
      y: Math.sin(progress * Math.PI * 2 * cycles) * amplitude
    });
  }

  function parabolaCurve(height) {
    return (progress) => ({
      x: progress,
      y: 4 * height * progress * (1 - progress)
    });
  }

  class LainPet {
    constructor(random = Math.random) {
      this.random = random;
      this.running = false;

      this.position = { x: 100, y: 100 };
      this.velocity = { x: 0, y: 0 };
      this.target = { x: 100, y: 100 };
      this.facing = "right";
      this.outfit = "default";
      this.mode = "idle";
      this.isDragging = false;

      this.idleRemaining = 0;
      this.dialogueRemaining = 0;
      this.dialogueText = null;
      this.expressionRemaining = 0;
      this.expressionAsset = null;

      this.resetMovement();
    }

    resetMovement() {
      this.movementTarget = null;
      this.movementStart = null;
      this.movementCurve = null;
      this.movementProgress = 0;
      this.movementDistance = 0;
      this.movementPath = "direct";
      this.movementRemaining = 0;
      this.movementTimedOut = false;
    }

    start() {
      this.running = true;
    }

    stop() {
      this.running = false;
      this.resetMovement();
      this.mode = "idle";
      this.isDragging = false;
      this.velocity = { x: 0, y: 0 };
      this.idleRemaining = 0;
      this.dialogueRemaining = 0;
      this.dialogueText = null;
      this.expressionRemaining = 0;
      this.expressionAsset = null;
    }

    isRunning() {
      return this.running;
    }

    wear(outfit) {
      if (assets[outfit]) this.outfit = outfit;
    }

    getPosition() {
      return this.running ? { ...this.position } : null;
    }

    snapshot() {
      return {
        position: { ...this.position },
        velocity: { ...this.velocity },
        target: { ...this.target },
        facing: this.facing,
        outfit: this.outfit,
        mode: this.mode,
        isDragging: this.isDragging,
        dialogue: {
          visible: this.dialogueRemaining > 0,
          text: this.dialogueText
        },
        expression: {
          visible: this.expressionRemaining > 0,
          asset: this.expressionAsset
        }
      };
    }

    beginDrag(position) {
      if (!this.running) return;
      this.cancelMovement();
      if (position) {
        this.position = { ...position };
        this.target = { ...position };
      }
      this.isDragging = true;
    }

    updateDrag(position) {
      if (!this.running || !this.isDragging) return;
      this.position = { ...position };
      this.target = { ...position };
      this.velocity = { x: 0, y: 0 };
    }

    endDrag() {
      if (!this.running) return;
      this.isDragging = false;
      this.cancelMovement();
    }

    moveTo(target) {
      this.requestMovement(target, "direct");
    }

    sineMoveTo(target) {
      this.requestMovement(target, "sine");
    }

    parabolicMoveTo(target, height = PARABOLA_HEIGHT) {
      this.requestMovement(target, "parabola", height);
    }

    requestMovement(target, path, height = PARABOLA_HEIGHT) {
      if (!this.running) return;
      this.target = { ...target };

      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
        return;
      }

      this.startMovement(target, path, height);
    }

    advance(elapsedMs, viewport) {
      if (!this.running) return this.snapshot();

      const elapsed = Number.isFinite(elapsedMs)
        ? clamp(elapsedMs, 0, MAX_FRAME_ELAPSED)
        : 0;

      this.expireTimers(elapsed);

      if (this.isDragging) return this.snapshot();

      if (this.mode === "walk") {
        this.moveToward(this.target, elapsed);
      } else {
        this.considerWalking(viewport, elapsed);
      }

      return this.snapshot();
    }

    expireTimers(elapsed) {
      if (this.idleRemaining > 0) {
        this.idleRemaining = Math.max(0, this.idleRemaining - elapsed);
      }

      if (this.movementRemaining > 0) {
        this.movementRemaining -= elapsed;
        if (this.movementRemaining <= 0) {
          this.movementRemaining = 0;
          this.movementTimedOut = true;
          this.mode = "idle";
          this.velocity = { x: 0, y: 0 };
          this.idleRemaining = IDLE_DURATION;
        }
      }

      if (this.dialogueRemaining > 0) {
        this.dialogueRemaining = Math.max(0, this.dialogueRemaining - elapsed);
        if (this.dialogueRemaining === 0) this.dialogueText = null;
      }

      if (this.expressionRemaining > 0) {
        this.expressionRemaining = Math.max(
          0,
          this.expressionRemaining - elapsed
        );
        if (this.expressionRemaining === 0) this.expressionAsset = null;
      }
    }

    isWithinTargetRadius(target) {
      const distance = vectorLength(subtractVectors(target, this.position));
      return distance <= TARGET_RADIUS + TARGET_TOLERANCE;
    }

    startMovement(target, path = "direct", pathHeight = PARABOLA_HEIGHT) {
      const sameTarget =
        this.movementTarget?.x === target.x &&
        this.movementTarget?.y === target.y;

      if (sameTarget && this.mode === "walk" && this.movementPath === path) {
        return true;
      }

      if (sameTarget && this.movementTimedOut && this.idleRemaining > 0) {
        return false;
      }

      this.movementTarget = { ...target };
      this.movementTimedOut = false;
      this.movementRemaining = MOVEMENT_TIMEOUT;
      this.target = { ...target };
      this.mode = "walk";

      if (path === "direct") {
        this.clearMovementPath();
        return true;
      }

      const start = { ...this.position };
      const displacement = subtractVectors(target, start);
      const distance = vectorLength(displacement);
      if (distance === 0) return false;

      const directionDegrees =
        (Math.atan2(displacement.y, displacement.x) * 180) / Math.PI;
      const localCurve =
        path === "sine"
          ? sineCurve(WAVE_AMPLITUDE, WAVE_CYCLES)
          : parabolaCurve(pathHeight);

      this.movementStart = start;
      this.movementProgress = 0;
      this.movementDistance = distance;
      this.movementPath = path;
      this.movementCurve = rotateCurve(
        (progress) => {
          const point = localCurve(progress);
          return { x: point.x * distance, y: point.y };
        },
        directionDegrees
      );

      return true;
    }

    clearMovementPath() {
      this.movementStart = null;
      this.movementCurve = null;
      this.movementProgress = 0;
      this.movementDistance = 0;
      this.movementPath = "direct";
    }

    cancelMovement() {
      this.resetMovement();
      this.mode = "idle";
      this.velocity = { x: 0, y: 0 };
      this.target = { ...this.position };
    }

    finishMovement() {
      this.resetMovement();
      this.mode = "idle";
      this.velocity = { x: 0, y: 0 };
      this.idleRemaining = IDLE_DURATION;
    }

    moveToward(target, elapsed) {
      if (!this.movementCurve) {
        this.moveDirectlyToward(target, elapsed);
        return;
      }

      if (!this.movementStart || this.movementDistance <= 0) {
        this.finishMovement();
        return;
      }

      const previousPosition = { ...this.position };
      const progressDelta =
        (WALK_SPEED * (elapsed / FRAME_DURATION)) / this.movementDistance;
      this.movementProgress = Math.min(1, this.movementProgress + progressDelta);

      const nextPosition =
        this.movementProgress >= 1
          ? { ...target }
          : addVectors(
              this.movementStart,
              this.movementCurve(this.movementProgress)
            );

      this.applyStep(subtractVectors(nextPosition, previousPosition));
      this.position = nextPosition;

      if (this.movementProgress >= 1) this.finishMovement();
    }

    moveDirectlyToward(target, elapsed) {
      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
        return;
      }

      const displacement = subtractVectors(target, this.position);
      const distance = vectorLength(displacement);
      if (distance === 0) {
        this.finishMovement();
        return;
      }

      const stepLength = Math.min(
        WALK_SPEED * (elapsed / FRAME_DURATION),
        distance - TARGET_RADIUS
      );
      const step = scaleVector(
        scaleVector(displacement, 1 / distance),
        stepLength
      );

      this.applyStep(step);
      this.position = addVectors(this.position, step);

      if (this.isWithinTargetRadius(target)) this.finishMovement();
    }

    applyStep(step) {
      this.velocity = step;
      if (step.x < 0) this.facing = "left";
      else if (step.x > 0) this.facing = "right";
    }

    considerWalking(viewport, elapsed) {
      if (this.idleRemaining > 0 || elapsed <= 0) return;

      const probability =
        1 - Math.pow(1 - WALK_CHANCE_PER_FRAME, elapsed / FRAME_DURATION);
      if (this.nextRandom() >= probability) return;

      const angle = this.nextRandom() * Math.PI * 2;
      const distance =
        WALK_MIN_DISTANCE +
        this.nextRandom() * (WALK_MAX_DISTANCE - WALK_MIN_DISTANCE);

      const maxX = Math.max(0, viewport.width - SPRITE_SIZE);
      const maxY = Math.max(0, viewport.height - SPRITE_SIZE);
      const target = {
        x: clamp(this.position.x + Math.cos(angle) * distance, 0, maxX),
        y: clamp(this.position.y + Math.sin(angle) * distance, 0, maxY)
      };

      const targetDistance = vectorLength(
        subtractVectors(target, this.position)
      );
      if (targetDistance <= TARGET_RADIUS) return;

      const pathRoll = this.nextRandom();
      let path = "direct";
      if (pathRoll < SINE_PATH_CHANCE) {
        path = "sine";
      } else if (pathRoll < SINE_PATH_CHANCE + PARABOLA_PATH_CHANCE) {
        path = "parabola";
      }

      this.startMovement(target, path);
    }

    speak(text) {
      if (!this.running) return;

      const message =
        text ?? dialogues[Math.floor(this.nextRandom() * dialogues.length)];
      this.dialogueText = message ?? "";
      this.dialogueRemaining = DIALOGUE_DURATION;

      this.expressionRemaining = 0;
      this.expressionAsset = null;
    }

    express() {
      if (!this.running || this.dialogueRemaining > 0) return;

      this.expressionAsset =
        this.outfit === "bear"
          ? assets.expressions.bear
          : assets.expressions.default;
      this.expressionRemaining = EXPRESSION_DURATION;
    }

    nextRandom() {
      const value = this.random();
      if (!Number.isFinite(value)) return 0.5;
      return clamp(value, 0, 0.999999999);
    }
  }

  class LainPetRenderer {
    constructor() {
      this.container = null;
      this.sprite = null;
      this.bubble = null;
      this.expression = null;
    }

    mount() {
      if (this.container) return;

      const container = document.createElement("div");
      container.className = "lain-layer";

      const sprite = document.createElement("img");
      sprite.className = "lain-sprite";
      sprite.alt = "";
      sprite.draggable = false;
      sprite.style.width = `${SPRITE_SIZE}px`;

      const bubble = document.createElement("div");
      bubble.className = "lain-bubble";
      bubble.style.width = `${BUBBLE_WIDTH}px`;

      const expression = document.createElement("img");
      expression.className = "lain-expression";
      expression.alt = "";
      expression.draggable = false;
      expression.style.width = `${EXPRESSION_SIZE}px`;

      container.append(sprite, bubble, expression);
      document.body.appendChild(container);

      this.container = container;
      this.sprite = sprite;
      this.bubble = bubble;
      this.expression = expression;
    }

    unmount() {
      this.container?.remove();
      this.container = null;
      this.sprite = null;
      this.bubble = null;
      this.expression = null;
    }

    getInteractiveElement() {
      return this.sprite;
    }

    render(snapshot) {
      if (!this.container) return;

      const { position } = snapshot;
      const centreX = position.x + SPRITE_SIZE / 2;

      this.sprite.style.left = `${position.x}px`;
      this.sprite.style.top = `${position.y}px`;
      this.sprite.classList.toggle("is-dragging", snapshot.isDragging);

      this.bubble.style.left = `${centreX - BUBBLE_WIDTH / 2}px`;
      this.bubble.style.top = `${position.y - BUBBLE_GAP}px`;

      this.expression.style.left = `${centreX - EXPRESSION_SIZE / 2}px`;
      this.expression.style.top = `${position.y - EXPRESSION_GAP}px`;

      this.renderSprite(snapshot);
      this.renderDialogue(snapshot.dialogue);
      this.renderExpression(snapshot.expression);
    }

    renderSprite(snapshot) {
      const outfit = assets[snapshot.outfit] ?? assets.default;
      const isMoving = snapshot.mode === "walk";
      const spriteUrl = isMoving
        ? snapshot.facing === "right"
          ? outfit.right
          : outfit.left
        : outfit.idle;

      const resolvedUrl = new URL(spriteUrl, document.baseURI).href;
      if (this.sprite.src !== resolvedUrl) this.sprite.src = spriteUrl;
    }

    renderDialogue(dialogue) {
      this.bubble.textContent = dialogue.text ?? "";
      this.bubble.classList.toggle("is-visible", dialogue.visible);
    }

    renderExpression(expression) {
      if (expression.asset) {
        const resolvedUrl = new URL(expression.asset, document.baseURI).href;
        if (this.expression.src !== resolvedUrl) {
          this.expression.src = expression.asset;
        }
      }
      this.expression.classList.toggle("is-visible", expression.visible);
    }
  }

  class LainPetPointerInput {
    constructor() {
      this.element = null;
      this.pet = null;
      this.grabOffset = { x: 0, y: 0 };
      this.onPointerDown = null;
      this.onPointerMove = null;
      this.onPointerEnd = null;
    }

    attach(element, pet) {
      this.detach();
      this.element = element;
      this.pet = pet;

      this.onPointerDown = (event) => {
        const { position } = pet.snapshot();

        this.grabOffset = {
          x: event.clientX - position.x,
          y: event.clientY - position.y
        };

        element.setPointerCapture?.(event.pointerId);
        pet.beginDrag(position);
        event.preventDefault();
      };

      this.onPointerMove = (event) => {
        if (!pet.snapshot().isDragging) return;
        event.preventDefault();

        const maxX = Math.max(0, window.innerWidth - SPRITE_SIZE);
        const maxY = Math.max(0, window.innerHeight - SPRITE_SIZE);

        pet.updateDrag({
          x: clamp(event.clientX - this.grabOffset.x, 0, maxX),
          y: clamp(event.clientY - this.grabOffset.y, 0, maxY)
        });
      };

      this.onPointerEnd = (event) => {
        if (!pet.snapshot().isDragging) return;
        pet.endDrag();
        if (element.hasPointerCapture?.(event.pointerId)) {
          element.releasePointerCapture(event.pointerId);
        }
      };

      element.addEventListener("pointerdown", this.onPointerDown);
      window.addEventListener("pointermove", this.onPointerMove, {
        passive: false
      });
      window.addEventListener("pointerup", this.onPointerEnd);
      window.addEventListener("pointercancel", this.onPointerEnd);
    }

    detach() {
      if (this.element && this.onPointerDown) {
        this.element.removeEventListener("pointerdown", this.onPointerDown);
      }
      if (this.onPointerMove) {
        window.removeEventListener("pointermove", this.onPointerMove);
      }
      if (this.onPointerEnd) {
        window.removeEventListener("pointerup", this.onPointerEnd);
        window.removeEventListener("pointercancel", this.onPointerEnd);
      }

      this.element = null;
      this.pet = null;
      this.onPointerDown = null;
      this.onPointerMove = null;
      this.onPointerEnd = null;
    }
  }

  class LainPetRuntime {
    constructor(
      pet = new LainPet(Math.random),
      renderer = new LainPetRenderer(),
      pointerInput = new LainPetPointerInput()
    ) {
      this.pet = pet;
      this.renderer = renderer;
      this.pointerInput = pointerInput;
      this.running = false;
      this.frameId = null;
      this.lastTimestamp = null;

      this.onFrame = (timestamp) => {
        if (!this.running) return;

        const elapsed =
          this.lastTimestamp === null
            ? 0
            : clamp(timestamp - this.lastTimestamp, 0, MAX_FRAME_ELAPSED);
        this.lastTimestamp = timestamp;

        const viewport = {
          width: window.innerWidth,
          height: window.innerHeight
        };

        this.renderer.render(this.pet.advance(elapsed, viewport));
        this.frameId = window.requestAnimationFrame(this.onFrame);
      };
    }

    start() {
      if (this.running) return;

      this.pet.start();
      this.renderer.mount();
      this.renderer.render(this.pet.snapshot());

      const element = this.renderer.getInteractiveElement();
      if (element) this.pointerInput.attach(element, this.pet);

      this.running = true;
      this.lastTimestamp = null;
      this.frameId = window.requestAnimationFrame(this.onFrame);
    }

    stop() {
      if (!this.running && this.frameId === null) return;

      this.running = false;
      if (this.frameId !== null) {
        window.cancelAnimationFrame(this.frameId);
        this.frameId = null;
      }

      this.lastTimestamp = null;
      this.pointerInput.detach();
      this.renderer.unmount();
      this.pet.stop();
    }

    isRunning() {
      return this.running;
    }

    getPet() {
      return this.pet;
    }
  }

  const runtime = new LainPetRuntime();
  const lainPet = runtime.getPet();

  let ambientInterval = null;
  let outfitInterval = null;
  let pendingStart = null;

  function startAmbientBehaviour() {
    if (ambientInterval !== null) return;

    ambientInterval = window.setInterval(() => {
      if (Math.random() < DIALOGUE_CHANCE) lainPet.speak();
      if (Math.random() < EXPRESSION_CHANCE) lainPet.express();
    }, AMBIENT_INTERVAL);

    outfitInterval = window.setInterval(() => {
      lainPet.wear(OUTFITS[Math.floor(Math.random() * OUTFITS.length)]);
    }, OUTFIT_INTERVAL);
  }

  function stopAmbientBehaviour() {
    if (ambientInterval !== null) {
      window.clearInterval(ambientInterval);
      ambientInterval = null;
    }
    if (outfitInterval !== null) {
      window.clearInterval(outfitInterval);
      outfitInterval = null;
    }
  }

  function start() {
    runtime.start();
    startAmbientBehaviour();
  }

  function stop() {
    if (pendingStart) {
      document.removeEventListener("DOMContentLoaded", pendingStart);
      pendingStart = null;
    }
    stopAmbientBehaviour();
    runtime.stop();
  }

  const api = {
    start,
    stop,
    setOutfit: (outfit) => lainPet.wear(outfit),
    speak: (text) => lainPet.speak(text),
    express: () => lainPet.express(),
    moveTo: (target) => lainPet.moveTo(target),
    sineMoveTo: (target) => lainPet.sineMoveTo(target),
    parabolicMoveTo: (target, height) =>
      height === undefined
        ? lainPet.parabolicMoveTo(target)
        : lainPet.parabolicMoveTo(target, height),
    getPosition: () => lainPet.getPosition()
  };

  function install() {
    window.LainPet?.stop();
    window.LainPet = api;
    window.Lain = api;

    if (document.readyState === "loading") {
      pendingStart = () => {
        pendingStart = null;
        start();
      };
      document.addEventListener("DOMContentLoaded", pendingStart, {
        once: true
      });
    } else {
      start();
    }
  }

  install();
})();
