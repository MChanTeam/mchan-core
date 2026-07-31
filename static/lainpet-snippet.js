(() => {
  var __defProp = Object.defineProperty;
  var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
  var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);

  // LainPet/classes/TimeoutRegistry.ts
  var TimeoutRegistry = class {
    constructor() {
      __publicField(this, "timeouts", /* @__PURE__ */ new Set());
    }
    schedule(callback, delay) {
      const timeoutId = window.setTimeout(() => {
        this.timeouts.delete(timeoutId);
        callback();
      }, delay);
      this.timeouts.add(timeoutId);
      return timeoutId;
    }
    cancel(timeoutId) {
      if (timeoutId === null) return;
      window.clearTimeout(timeoutId);
      this.timeouts.delete(timeoutId);
    }
    clear() {
      for (const timeout of this.timeouts) {
        window.clearTimeout(timeout);
      }
      this.timeouts.clear();
    }
  };

  // LainPet/classes/FlyingMisc.ts
  var MISC_SIZE = {
    crow: 120,
    girl: 100
  };
  var FlyingMisc = class {
    constructor(assets2) {
      __publicField(this, "assets", assets2);
      __publicField(this, "items", /* @__PURE__ */ new Set());
      __publicField(this, "timeouts", new TimeoutRegistry());
    }
    spawn(type) {
      const asset = this.assets[type];
      if (!asset) return false;
      const size = MISC_SIZE[type];
      const item = document.createElement("img");
      item.src = asset;
      item.style.cssText = `
      position: fixed;
      width: ${size}px;
      z-index: 9998;
      pointer-events: none;
      transition: left 8s linear;
      top: ${Math.random() * Math.max(0, window.innerHeight - size)}px;
    `;
      const startX = Math.random() < 0.5 ? -size : window.innerWidth + size;
      const movingRight = startX < 0;
      item.style.left = `${startX}px`;
      item.style.transform = type === "crow" ? movingRight ? "scaleX(1)" : "scaleX(-1)" : movingRight ? "scaleX(-1)" : "scaleX(1)";
      document.body.appendChild(item);
      this.items.add(item);
      this.schedule(() => {
        if (!this.items.has(item)) return;
        item.style.left = `${movingRight ? window.innerWidth + size : -size}px`;
      }, 100);
      this.schedule(() => {
        this.items.delete(item);
        item.remove();
      }, 9e3);
      return true;
    }
    stop() {
      this.clearTimeouts();
      for (const item of this.items) {
        item.remove();
      }
      this.items.clear();
    }
    schedule(callback, delay) {
      this.timeouts.schedule(callback, delay);
    }
    clearTimeouts() {
      this.timeouts.clear();
    }
  };

  // LainPet/data/assets.ts
  var assets = {
    default: {
      idle: "/static/lainpet-assets/1.png",
      right: "/static/lainpet-assets/lainwalk1.gif",
      left: "/static/lainpet-assets/lainwalk2.gif"
    },
    school: {
      idle: "/static/lainpet-assets/115.png",
      right: "/static/lainpet-assets/lainwalk3.gif",
      left: "/static/lainpet-assets/lainwalk4.gif",
      event: "/static/lainpet-assets/lainburn.gif"
    },
    pink: {
      idle: "/static/lainpet-assets/116.png",
      right: "/static/lainpet-assets/lainwalk5.gif",
      left: "/static/lainpet-assets/lainwalk6.gif",
      event: "/static/lainpet-assets/laindance.gif"
    },
    bear: {
      idle: "/static/lainpet-assets/117.png",
      right: "/static/lainpet-assets/lainwalk7.gif",
      left: "/static/lainpet-assets/lainwalk8.gif",
      event: "/static/lainpet-assets/lainroll.gif"
    },
    home: {
      idle: "/static/lainpet-assets/118.png",
      right: "/static/lainpet-assets/lainwalk9.gif",
      left: "/static/lainpet-assets/lainwalk10.gif"
    },
    misc: {
      crow: "/static/lainpet-assets/crow.gif",
      girl: "/static/lainpet-assets/flyinggirl.gif",
      navi: [
        "/static/lainpet-assets/navi1.gif",
        "/static/lainpet-assets/navi2.gif",
        "/static/lainpet-assets/navi3.gif"
      ],
      exp1: "/static/lainpet-assets/expression1.gif",
      exp2: "/static/lainpet-assets/expression2.gif"
    }
  };
  var dialogues = [
    "Present day. Present time. Hahahaha!",
    "Why don't you come to the Wired?",
    "No matter where you are, everyone is always connected.",
    "No matter where you are, everyone is always connected.",
    "You're wrong.",
    "Don't worry. I'm still me.",
    "check out mxrza.xyz",
    "Let's All Love Lain.",
    "Why are you crying Lain",
    "The real world isn't real at all.",
    "also check out kuumin (website pending)"
  ];

  // LainPet/types/Vector2.ts
  function add(a, b) {
    return {
      x: a.x + b.x,
      y: a.y + b.y
    };
  }
  function subtract(a, b) {
    return {
      x: a.x - b.x,
      y: a.y - b.y
    };
  }
  function scale(a, scalar) {
    return {
      x: a.x * scalar,
      y: a.y * scalar
    };
  }
  function magnitude(vector) {
    return Math.hypot(vector.x, vector.y);
  }

  // LainPet/types/ParametricCurve.ts
  function rotateFunction(curve, degrees) {
    const radians = degrees * Math.PI / 180;
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
  function sineFunction(amplitude, cycles = 1) {
    return (progress) => ({
      x: progress,
      y: Math.sin(progress * Math.PI * 2 * cycles) * amplitude
    });
  }
  function parabolaFunction(height) {
    return (progress) => ({
      x: progress,
      y: 4 * height * progress * (1 - progress)
    });
  }

  // LainPet/classes/LainPet.ts
  var SPRITE_SIZE = {
    normal: 100,
    event: 200
  };
  var IDLE_DURATION = 5e3;
  var MOVEMENT_TIMEOUT = 1e4;
  var WALK_RADIUS = 1e3;
  var WALK_MIN_DISTANCE = 100;
  var TARGET_RADIUS = 5;
  var TARGET_TOLERANCE = 1e-6;
  var FRAME_DURATION = 30;
  var MAX_ELAPSED = 100;
  var WAVE_AMPLITUDE = 40;
  var WAVE_CYCLES = 1.5;
  var PARABOLA_HEIGHT = 80;
  var AUTONOMOUS_WALK_CHANCE = 5e-3;
  var AUTONOMOUS_SINE_CHANCE = 0.15;
  var AUTONOMOUS_PARABOLA_CHANCE = 0.15;
  var SUGAR_RUSH_MIN_DISTANCE = 180;
  var SUGAR_RUSH_MAX_DISTANCE = 360;
  var SUGAR_RUSH_SINE_CHANCE = 0.5;
  var SUGAR_RUSH_SPEED = 13;
  var EVENT_DURATIONS = {
    bear: 8e3,
    school: 3e3,
    pink: 1e4
  };
  var LainPet = class {
    constructor(options = {}) {
      __publicField(this, "running", false);
      __publicField(this, "random");
      __publicField(this, "movementTarget", null);
      __publicField(this, "movementStart", null);
      __publicField(this, "movementProgress", 0);
      __publicField(this, "movementDistance", 0);
      __publicField(this, "movementCurve", null);
      __publicField(this, "movementPath", "direct");
      __publicField(this, "movementPathHeight", 0);
      __publicField(this, "movementRemaining", 0);
      __publicField(this, "movementTimedOut", false);
      __publicField(this, "idleRemaining", 0);
      __publicField(this, "expression", { remaining: 0, invocation: 0 });
      __publicField(this, "dialogue", { remaining: 0, invocation: 0 });
      __publicField(this, "sugarRushState", { remaining: 0, invocation: 0 });
      __publicField(this, "event", { remaining: 0, invocation: 0 });
      __publicField(this, "eventOutfit", null);
      __publicField(this, "dialogueText", null);
      __publicField(this, "expressionAsset", null);
      __publicField(this, "facing", "right");
      __publicField(this, "state", {
        position: { x: 100, y: 100 },
        velocity: { x: 0, y: 0 },
        target: { x: 100, y: 100 },
        speed: 3,
        outfit: "default",
        mode: "idle",
        isDragging: false,
        eventActive: false,
        sugarRush: false
      });
      this.random = typeof options === "function" ? options : options.random ?? (() => 0.5);
    }
    start() {
      this.running = true;
    }
    stop() {
      this.running = false;
      this.resetMovement();
      this.state.position = { x: 100, y: 100 };
      this.state.velocity = { x: 0, y: 0 };
      this.state.target = { x: 100, y: 100 };
      this.state.mode = "idle";
      this.state.isDragging = false;
      this.state.eventActive = false;
      this.state.sugarRush = false;
      this.idleRemaining = 0;
      this.expression = { remaining: 0, invocation: this.expression.invocation + 1 };
      this.dialogue = { remaining: 0, invocation: this.dialogue.invocation + 1 };
      this.sugarRushState = {
        remaining: 0,
        invocation: this.sugarRushState.invocation + 1
      };
      this.event = { remaining: 0, invocation: this.event.invocation + 1 };
      this.eventOutfit = null;
      this.dialogueText = null;
      this.expressionAsset = null;
      this.facing = "right";
    }
    isRunning() {
      return this.running;
    }
    wear(outfit) {
      this.state.outfit = outfit;
    }
    forceRoll() {
      this.triggerSpecialEvent("bear");
    }
    forceBurn() {
      this.triggerSpecialEvent("school");
    }
    forceDance() {
      this.triggerSpecialEvent("pink");
    }
    sugarRush() {
      this.triggerSugarRush();
    }
    express() {
      this.triggerExpression();
    }
    speak(text) {
      this.showDialogue(text);
    }
    specialEvent() {
      this.triggerSpecialEvent();
    }
    getPosition() {
      if (!this.running) return null;
      return { ...this.state.position };
    }
    snapshot() {
      const dialogue = {
        visible: this.dialogue.remaining > 0,
        text: this.dialogueText
      };
      const expression = {
        visible: this.expression.remaining > 0,
        asset: this.expressionAsset
      };
      return {
        ...this.state,
        position: { ...this.state.position },
        velocity: { ...this.state.velocity },
        target: { ...this.state.target },
        facing: this.facing,
        dialogue,
        eventOutfit: this.eventOutfit,
        expression
      };
    }
    getSnapshot() {
      return this.snapshot();
    }
    beginDrag(position) {
      if (!this.running) return;
      this.cancelMovement();
      if (position) {
        this.state.position = { ...position };
        this.state.target = { ...position };
      }
      this.state.isDragging = true;
    }
    updateDrag(position) {
      if (!this.running || !this.state.isDragging) return;
      this.state.position = { ...position };
      this.state.target = { ...position };
      this.state.velocity = { x: 0, y: 0 };
    }
    endDrag() {
      if (!this.running) return;
      this.state.isDragging = false;
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
      if (!this.running || this.state.sugarRush) return;
      this.state.target = { ...target };
      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
        return;
      }
      this.startMovement(target, MOVEMENT_TIMEOUT, path, height);
    }
    /** Advance behavior by at most 100ms; the host owns the frame clock. */
    advance(elapsedMs, viewport, random = this.random) {
      if (!this.running) return this.snapshot();
      const previousRandom = this.random;
      this.random = random;
      try {
        const elapsed = Number.isFinite(elapsedMs) ? Math.max(0, Math.min(MAX_ELAPSED, elapsedMs)) : 0;
        this.expireTimedStates(elapsed);
        if (this.state.isDragging) return this.snapshot();
        if (this.state.eventActive) {
          this.moveToEventCenter(viewport, elapsed);
        } else if (this.state.sugarRush) {
          this.moveDuringSugarRush(viewport, elapsed);
        } else if (this.state.mode === "walk") {
          this.moveTowardTarget(elapsed);
        } else {
          this.tryToStartWalking(viewport, elapsed);
        }
        return this.snapshot();
      } finally {
        this.random = previousRandom;
      }
    }
    expireTimedStates(elapsed) {
      if (this.idleRemaining > 0) {
        this.idleRemaining = Math.max(0, this.idleRemaining - elapsed);
      }
      if (this.movementRemaining > 0) {
        this.movementRemaining -= elapsed;
        if (this.movementRemaining <= 0) {
          this.movementRemaining = 0;
          this.movementTimedOut = true;
          this.state.mode = "idle";
          this.state.velocity = { x: 0, y: 0 };
          this.idleRemaining = IDLE_DURATION;
        }
      }
      if (this.expression.remaining > 0) {
        this.expression.remaining = Math.max(0, this.expression.remaining - elapsed);
        if (this.expression.remaining === 0) this.expressionAsset = null;
      }
      if (this.dialogue.remaining > 0) {
        this.dialogue.remaining = Math.max(0, this.dialogue.remaining - elapsed);
        if (this.dialogue.remaining === 0) this.dialogueText = null;
      }
      if (this.sugarRushState.remaining > 0) {
        this.sugarRushState.remaining = Math.max(
          0,
          this.sugarRushState.remaining - elapsed
        );
        if (this.sugarRushState.remaining === 0) {
          this.state.sugarRush = false;
          this.finishMovement();
        }
      }
      if (this.event.remaining > 0) {
        this.event.remaining = Math.max(0, this.event.remaining - elapsed);
        if (this.event.remaining === 0) {
          this.state.eventActive = false;
          this.eventOutfit = null;
          this.finishMovement();
        }
      }
    }
    isWithinTargetRadius(target) {
      return magnitude(subtract(target, this.state.position)) <= TARGET_RADIUS + TARGET_TOLERANCE;
    }
    startMovement(target, timeout = MOVEMENT_TIMEOUT, path = "direct", pathHeight = PARABOLA_HEIGHT) {
      const sameTarget = this.movementTarget?.x === target.x && this.movementTarget?.y === target.y;
      const samePath = this.movementPath === path && (path !== "parabola" || this.movementPathHeight === pathHeight);
      if (sameTarget && this.state.mode === "walk" && samePath) return true;
      if (sameTarget && this.movementTimedOut && this.idleRemaining > 0) {
        return false;
      }
      this.movementTarget = { ...target };
      this.movementTimedOut = false;
      this.movementRemaining = timeout;
      this.state.target = { ...target };
      this.state.mode = "walk";
      if (path === "direct") {
        this.clearMovementPath();
        return true;
      }
      const start2 = { ...this.state.position };
      const displacement = subtract(target, start2);
      const distance = magnitude(displacement);
      if (distance === 0) return false;
      const directionDegrees = Math.atan2(displacement.y, displacement.x) * 180 / Math.PI;
      const localCurve = path === "sine" ? sineFunction(WAVE_AMPLITUDE, WAVE_CYCLES) : parabolaFunction(pathHeight);
      this.movementStart = start2;
      this.movementProgress = 0;
      this.movementDistance = distance;
      this.movementPath = path;
      this.movementPathHeight = pathHeight;
      this.movementCurve = rotateFunction(
        (progress) => {
          const point = localCurve(progress);
          return {
            x: point.x * distance,
            y: point.y
          };
        },
        directionDegrees
      );
      return true;
    }
    clearMovementPath() {
      this.movementStart = null;
      this.movementProgress = 0;
      this.movementDistance = 0;
      this.movementCurve = null;
      this.movementPath = "direct";
      this.movementPathHeight = 0;
    }
    cancelMovement() {
      this.movementRemaining = 0;
      this.movementTarget = null;
      this.clearMovementPath();
      this.movementTimedOut = false;
      this.state.mode = "idle";
      this.state.velocity = { x: 0, y: 0 };
      this.state.target = { ...this.state.position };
    }
    finishMovement() {
      this.movementRemaining = 0;
      this.movementTarget = null;
      this.clearMovementPath();
      this.movementTimedOut = false;
      this.state.mode = "idle";
      this.state.velocity = { x: 0, y: 0 };
      this.idleRemaining = IDLE_DURATION;
    }
    resetMovement() {
      this.movementRemaining = 0;
      this.movementTarget = null;
      this.clearMovementPath();
      this.movementTimedOut = false;
    }
    moveToEventCenter(viewport, elapsed) {
      const size = SPRITE_SIZE.event;
      const center = {
        x: (viewport.width - size) / 2,
        y: (viewport.height - size) / 2
      };
      if (this.isWithinTargetRadius(center)) {
        this.finishMovement();
        return;
      }
      if (!this.startMovement(center)) return;
      this.moveToward(center, elapsed);
    }
    moveTowardTarget(elapsed) {
      this.moveToward(this.state.target, elapsed);
    }
    moveToward(target, elapsed) {
      if (!this.movementCurve) {
        if (this.isWithinTargetRadius(target)) {
          this.finishMovement();
          return;
        }
        const displacement = subtract(target, this.state.position);
        const distance = magnitude(displacement);
        if (distance === 0) {
          this.finishMovement();
          return;
        }
        const step2 = scale(
          scale(displacement, 1 / distance),
          Math.min(
            this.state.speed * (elapsed / FRAME_DURATION),
            distance - TARGET_RADIUS
          )
        );
        this.state.velocity = step2;
        this.state.position = add(this.state.position, step2);
        if (step2.x < 0) this.facing = "left";
        else if (step2.x > 0) this.facing = "right";
        if (this.isWithinTargetRadius(target)) this.finishMovement();
        return;
      }
      if (!this.movementStart || this.movementDistance <= 0) {
        this.finishMovement();
        return;
      }
      const previousPosition = { ...this.state.position };
      const movementSpeed = this.state.sugarRush ? SUGAR_RUSH_SPEED : this.state.speed;
      const progressDelta = movementSpeed * (elapsed / FRAME_DURATION) / this.movementDistance;
      this.movementProgress = Math.min(
        1,
        this.movementProgress + progressDelta
      );
      const nextPosition = this.movementProgress >= 1 ? { ...target } : add(this.movementStart, this.movementCurve(this.movementProgress));
      const step = subtract(nextPosition, previousPosition);
      this.state.velocity = step;
      this.state.position = nextPosition;
      if (step.x < 0) this.facing = "left";
      else if (step.x > 0) this.facing = "right";
      if (this.movementProgress >= 1) this.finishMovement();
    }
    startSugarRushMovement(viewport) {
      const maxX = Math.max(0, viewport.width - SPRITE_SIZE.normal);
      const maxY = Math.max(0, viewport.height - SPRITE_SIZE.normal);
      const angle = this.nextRandom() * Math.PI * 2;
      const distance = SUGAR_RUSH_MIN_DISTANCE + this.nextRandom() * (SUGAR_RUSH_MAX_DISTANCE - SUGAR_RUSH_MIN_DISTANCE);
      const unclampedTarget = {
        x: this.state.position.x + Math.cos(angle) * distance,
        y: this.state.position.y + Math.sin(angle) * distance
      };
      const target = {
        x: Math.max(0, Math.min(unclampedTarget.x, maxX)),
        y: Math.max(0, Math.min(unclampedTarget.y, maxY))
      };
      if (magnitude(subtract(target, this.state.position)) <= TARGET_RADIUS) {
        return false;
      }
      const path = this.nextRandom() < SUGAR_RUSH_SINE_CHANCE ? "sine" : "parabola";
      return this.startMovement(target, MOVEMENT_TIMEOUT, path);
    }
    moveDuringSugarRush(viewport, elapsed) {
      if (!this.movementCurve) {
        this.startSugarRushMovement(viewport);
      }
      if (this.movementCurve) {
        this.moveTowardTarget(elapsed);
      }
      const maxX = Math.max(0, viewport.width - SPRITE_SIZE.normal);
      const maxY = Math.max(0, viewport.height - SPRITE_SIZE.normal);
      this.state.position.x = Math.max(0, Math.min(this.state.position.x, maxX));
      this.state.position.y = Math.max(0, Math.min(this.state.position.y, maxY));
      if (this.state.velocity.x < 0) this.facing = "left";
      else if (this.state.velocity.x > 0) this.facing = "right";
    }
    tryToStartWalking(viewport, elapsed) {
      if (this.idleRemaining > 0 || elapsed <= 0) return;
      const probability = 1 - Math.pow(
        1 - AUTONOMOUS_WALK_CHANCE,
        elapsed / FRAME_DURATION
      );
      if (this.nextRandom() >= probability) return;
      const angle = this.nextRandom() * Math.PI * 2;
      const distance = WALK_MIN_DISTANCE + this.nextRandom() * (WALK_RADIUS - WALK_MIN_DISTANCE);
      const maxX = Math.max(0, viewport.width - SPRITE_SIZE.normal);
      const maxY = Math.max(0, viewport.height - SPRITE_SIZE.normal);
      const unclampedTarget = {
        x: this.state.position.x + Math.cos(angle) * distance,
        y: this.state.position.y + Math.sin(angle) * distance
      };
      const target = {
        x: Math.max(0, Math.min(unclampedTarget.x, maxX)),
        y: Math.max(0, Math.min(unclampedTarget.y, maxY))
      };
      const targetDistance = magnitude(subtract(target, this.state.position));
      if (targetDistance <= TARGET_RADIUS || targetDistance > WALK_RADIUS) return;
      const pathRoll = this.nextRandom();
      const path = pathRoll < AUTONOMOUS_SINE_CHANCE ? "sine" : pathRoll < AUTONOMOUS_SINE_CHANCE + AUTONOMOUS_PARABOLA_CHANCE ? "parabola" : "direct";
      this.startMovement(target, MOVEMENT_TIMEOUT, path);
    }
    triggerExpression() {
      if (!this.running || this.state.eventActive) return;
      this.expressionAsset = this.state.outfit === "bear" ? assets.misc.exp2 : assets.misc.exp1;
      this.expression = {
        remaining: 3e3,
        invocation: this.expression.invocation + 1
      };
    }
    triggerSpecialEvent(outfit = this.state.outfit) {
      if (!this.running || this.state.eventActive || this.state.sugarRush) return;
      const eventAsset = assets[outfit]?.event;
      if (!eventAsset) return;
      this.event = {
        remaining: EVENT_DURATIONS[outfit] ?? 1e4,
        invocation: this.event.invocation + 1
      };
      this.eventOutfit = outfit;
      this.state.eventActive = true;
    }
    triggerSugarRush() {
      if (!this.running || this.state.eventActive) return;
      this.sugarRushState = {
        remaining: 5e3,
        invocation: this.sugarRushState.invocation + 1
      };
      this.cancelMovement();
      this.state.sugarRush = true;
      this.state.velocity = scale(
        {
          x: this.nextRandom() > 0.5 ? 1 : -1,
          y: this.nextRandom() > 0.5 ? 1 : -1
        },
        10
      );
      if (this.state.velocity.x < 0) this.facing = "left";
      else if (this.state.velocity.x > 0) this.facing = "right";
    }
    showDialogue(text) {
      if (!this.running) return;
      const message = text ?? dialogues[Math.floor(this.nextRandom() * dialogues.length)];
      this.dialogueText = message ?? "";
      this.dialogue = {
        remaining: 4e3,
        invocation: this.dialogue.invocation + 1
      };
    }
    nextRandom() {
      const value = this.random();
      if (!Number.isFinite(value)) return 0.5;
      return Math.max(0, Math.min(0.999999999, value));
    }
  };

  // LainPet/classes/LainPetPointerInput.ts
  var LainPetPointerInput = class {
    constructor() {
      __publicField(this, "element", null);
      __publicField(this, "moveHandler", null);
      __publicField(this, "endHandler", null);
    }
    attach(element, pet) {
      this.detach();
      this.element = element;
      const onPointerMove = (event) => {
        if (!pet.snapshot().isDragging) return;
        event.preventDefault();
        pet.updateDrag({ x: event.clientX - 50, y: event.clientY - 50 });
      };
      const onPointerEnd = (event) => {
        if (!pet.snapshot().isDragging) return;
        event.preventDefault();
        pet.endDrag();
        if (this.element?.hasPointerCapture(event.pointerId)) {
          this.element.releasePointerCapture(event.pointerId);
        }
      };
      const onPointerDown = (event) => {
        event.preventDefault();
        this.element?.setPointerCapture(event.pointerId);
        pet.beginDrag();
      };
      window.addEventListener("pointermove", onPointerMove, { passive: false });
      window.addEventListener("pointerup", onPointerEnd, { passive: false });
      window.addEventListener("pointercancel", onPointerEnd, { passive: false });
      this.moveHandler = onPointerMove;
      this.endHandler = onPointerEnd;
      element.onpointerdown = onPointerDown;
    }
    detach() {
      if (this.element?.onpointerdown) this.element.onpointerdown = null;
      if (this.moveHandler) {
        window.removeEventListener("pointermove", this.moveHandler);
        this.moveHandler = null;
      }
      if (this.endHandler) {
        window.removeEventListener("pointerup", this.endHandler);
        window.removeEventListener("pointercancel", this.endHandler);
        this.endHandler = null;
      }
      this.element = null;
    }
  };

  // LainPet/classes/LainPetRenderer.ts
  var NORMAL_SIZE = 100;
  var EVENT_SIZE = 200;
  var LainPetRenderer = class {
    constructor() {
      __publicField(this, "container", null);
      __publicField(this, "lainSprite", null);
      __publicField(this, "bubble", null);
      __publicField(this, "expression", null);
    }
    mount() {
      if (this.container) return;
      const container = document.createElement("div");
      const lainSprite = document.createElement("img");
      const bubble = document.createElement("div");
      const expression = document.createElement("img");
      container.style.cssText = `
      position: fixed;
      z-index: 9999;
      pointer-events: none;
      top: 0;
      left: 0;
      width: 100vw;
      height: 100vh;
    `;
      lainSprite.style.cssText = `
      position: absolute;
      width: ${NORMAL_SIZE}px;
      pointer-events: auto;
      cursor: grab;
      transition: filter 0.2s;
      object-fit: contain;
    `;
      bubble.style.cssText = `
      position: absolute;
      background: white;
      color: black;
      border: 2px solid black;
      padding: 8px;
      border-radius: 10px;
      font-family: monospace;
      font-size: 12px;
      opacity: 0;
      transition: opacity 0.5s;
      width: 150px;
      text-align: center;
      z-index: 10000;
      pointer-events: none;
    `;
      expression.style.cssText = `
      position: absolute;
      width: 50px;
      opacity: 0;
      transition: opacity 0.3s;
      z-index: 10001;
      pointer-events: none;
    `;
      container.append(lainSprite, bubble, expression);
      document.body.appendChild(container);
      this.container = container;
      this.lainSprite = lainSprite;
      this.bubble = bubble;
      this.expression = expression;
    }
    unmount() {
      this.container?.remove();
      this.container = null;
      this.lainSprite = null;
      this.bubble = null;
      this.expression = null;
    }
    getInteractiveElement() {
      return this.lainSprite;
    }
    render(snapshot, timestamp = 0) {
      if (!this.container || !this.lainSprite || !this.bubble || !this.expression) {
        return;
      }
      const size = snapshot.eventActive ? EVENT_SIZE : NORMAL_SIZE;
      const { position } = snapshot;
      const eventAssets = assets[snapshot.eventOutfit ?? snapshot.outfit];
      const outfitAssets = assets[snapshot.outfit];
      const isMoving = snapshot.mode === "walk" || snapshot.sugarRush;
      const spriteUrl = snapshot.eventActive ? eventAssets.event ?? eventAssets.idle : isMoving ? snapshot.facing === "right" ? outfitAssets.right : outfitAssets.left : outfitAssets.idle;
      this.lainSprite.style.width = `${size}px`;
      this.lainSprite.style.left = `${position.x}px`;
      this.lainSprite.style.top = `${position.y}px`;
      this.bubble.style.left = `${position.x + size / 2 - 75}px`;
      this.bubble.style.top = `${position.y - 50}px`;
      this.expression.style.left = `${position.x + size / 2 - 25}px`;
      this.expression.style.top = `${position.y - 40}px`;
      this.lainSprite.style.cursor = snapshot.isDragging ? "grabbing" : "grab";
      const resolvedSpriteUrl = new URL(spriteUrl, document.baseURI).href;
      if (this.lainSprite.src !== resolvedSpriteUrl) {
        this.lainSprite.src = spriteUrl;
      }
      this.lainSprite.style.filter = snapshot.sugarRush ? `hue-rotate(${timestamp % 360}deg) brightness(1.2)` : "";
      const dialogue = snapshot.dialogue;
      this.bubble.textContent = dialogue.text ?? "";
      this.bubble.style.opacity = dialogue.visible ? "1" : "0";
      const expressionState = snapshot.expression;
      if (expressionState.asset && this.expression.src !== expressionState.asset) {
        this.expression.src = expressionState.asset;
      }
      this.expression.style.opacity = expressionState.visible ? "1" : "0";
    }
  };

  // LainPet/classes/LainPetRuntime.ts
  var MAX_ELAPSED_MS = 100;
  var LainPetRuntime = class {
    constructor(pet = new LainPet(Math.random), renderer = new LainPetRenderer(), pointerInput = new LainPetPointerInput()) {
      __publicField(this, "pet", pet);
      __publicField(this, "renderer");
      __publicField(this, "pointerInput");
      __publicField(this, "frameId", null);
      __publicField(this, "lastTimestamp", null);
      __publicField(this, "running", false);
      __publicField(this, "onFrame", (timestamp) => {
        if (!this.running) return;
        const elapsed = this.lastTimestamp === null ? 0 : Math.min(MAX_ELAPSED_MS, Math.max(0, timestamp - this.lastTimestamp));
        this.lastTimestamp = timestamp;
        const viewport = {
          width: window.innerWidth,
          height: window.innerHeight
        };
        const snapshot = this.pet.advance(elapsed, viewport, Math.random);
        this.renderer.render(snapshot, timestamp);
        this.frameId = window.requestAnimationFrame(this.onFrame);
      });
      this.renderer = renderer;
      this.pointerInput = pointerInput;
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
    snapshot() {
      return this.pet.snapshot();
    }
    getPet() {
      return this.pet;
    }
  };

  // LainPet/classes/Navi.ts
  var NAVI_SIZE = 120;
  var NAVI_LANDED_TOP_OFFSET = 150;
  var Navi = class {
    constructor(assetUrls) {
      __publicField(this, "assetUrls", assetUrls);
      __publicField(this, "item", null);
      __publicField(this, "landed", false);
      __publicField(this, "timeouts", new TimeoutRegistry());
    }
    drop() {
      if (this.item || this.assetUrls.length === 0) {
        return false;
      }
      const item = document.createElement("img");
      item.src = this.assetUrls[Math.floor(Math.random() * this.assetUrls.length)];
      item.style.cssText = `
      position: fixed;
      width: ${NAVI_SIZE}px;
      z-index: 9997;
      pointer-events: none;
      transition: top 6s linear;
      top: -150px;
    `;
      item.style.left = `${Math.random() * (window.innerWidth - NAVI_SIZE)}px`;
      document.body.appendChild(item);
      this.item = item;
      this.landed = false;
      this.schedule(() => {
        if (this.item !== item) return;
        item.style.top = `${window.innerHeight - NAVI_LANDED_TOP_OFFSET}px`;
      }, 100);
      this.schedule(() => {
        if (this.item !== item) return;
        this.landed = true;
      }, 6e3);
      this.schedule(() => {
        if (this.item !== item) return;
        this.removeItem();
      }, 15e3);
      return true;
    }
    isLanded() {
      return this.item !== null && this.landed;
    }
    getPosition() {
      if (!this.item) {
        return null;
      }
      const x = Number.parseFloat(this.item.style.left);
      const y = Number.parseFloat(this.item.style.top);
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        return null;
      }
      return { x, y };
    }
    collect() {
      if (!this.item) {
        return false;
      }
      this.clearTimeouts();
      this.removeItem();
      return true;
    }
    stop() {
      this.clearTimeouts();
      this.removeItem();
    }
    schedule(callback, delay) {
      this.timeouts.schedule(callback, delay);
    }
    clearTimeouts() {
      this.timeouts.clear();
    }
    removeItem() {
      this.item?.remove();
      this.item = null;
      this.landed = false;
    }
  };

  // snippet.ts
  var OUTFITS = ["default", "school", "pink", "bear", "home"];
  var runtime = new LainPetRuntime();
  var lainPet = runtime.getPet();
  var navi = new Navi(assets.misc.navi);
  var flyingMisc = new FlyingMisc({
    crow: assets.misc.crow,
    girl: assets.misc.girl
  });
  var ambientInterval = null;
  var outfitInterval = null;
  var naviInterval = null;
  var pendingStart = null;
  function updateNaviInteraction() {
    if (!navi.isLanded()) return;
    const lainPosition = lainPet.getPosition();
    const naviPosition = navi.getPosition();
    if (!lainPosition || !naviPosition) return;
    lainPet.moveTo(naviPosition);
    const lainCenter = {
      x: lainPosition.x + 50,
      y: lainPosition.y + 50
    };
    const naviCenter = {
      x: naviPosition.x + 60,
      y: naviPosition.y + 60
    };
    if (magnitude(subtract(lainCenter, naviCenter)) >= 30) return;
    if (!navi.collect()) return;
    lainPet.sugarRush();
    lainPet.speak("NAVI COLLECTED.");
  }
  function startAmbientBehavior() {
    if (ambientInterval !== null) return;
    ambientInterval = window.setInterval(() => {
      if (Math.random() < 0.2) lainPet.speak();
      if (Math.random() < 0.2) lainPet.express();
      if (Math.random() < 0.1) {
        flyingMisc.spawn(Math.random() < 0.5 ? "crow" : "girl");
      }
      if (Math.random() < 0.05) navi.drop();
    }, 15e3);
    outfitInterval = window.setInterval(() => {
      const randomOutfit = OUTFITS[Math.floor(Math.random() * OUTFITS.length)];
      lainPet.wear(randomOutfit);
      if (Math.random() < 0.4) {
        lainPet.specialEvent();
      }
    }, 6e4);
    naviInterval = window.setInterval(updateNaviInteraction, 30);
  }
  function stopAmbientBehavior() {
    if (ambientInterval !== null) {
      window.clearInterval(ambientInterval);
      ambientInterval = null;
    }
    if (outfitInterval !== null) {
      window.clearInterval(outfitInterval);
      outfitInterval = null;
    }
    if (naviInterval !== null) {
      window.clearInterval(naviInterval);
      naviInterval = null;
    }
  }
  function start() {
    runtime.start();
    startAmbientBehavior();
    console.log(
      "%c Lain Pet by realmxrza | standalone browser snippet started ",
      "background: #000; color: #f0f; font-weight: bold; font-size: 14px;"
    );
  }
  function stop() {
    if (pendingStart) {
      document.removeEventListener("DOMContentLoaded", pendingStart);
      pendingStart = null;
    }
    stopAmbientBehavior();
    navi.stop();
    flyingMisc.stop();
    runtime.stop();
    console.log(
      "%c Lain Pet by realmxrza | standalone browser snippet stopped ",
      "background: #000; color: #f0f; font-weight: bold; font-size: 14px;"
    );
  }
  var api = {
    start,
    stop,
    forceRoll: () => lainPet.forceRoll(),
    forceBurn: () => lainPet.forceBurn(),
    forceDance: () => lainPet.forceDance(),
    specialEvent: () => lainPet.specialEvent(),
    dropNavi: () => navi.drop(),
    sugarRush: () => lainPet.sugarRush(),
    setOutfit: (outfit) => lainPet.wear(outfit),
    spawnCrow: () => flyingMisc.spawn("crow"),
    spawnGirl: () => flyingMisc.spawn("girl"),
    speak: (text) => lainPet.speak(text),
    express: () => lainPet.express(),
    moveTo: (target) => lainPet.moveTo(target),
    sineMoveTo: (target) => lainPet.sineMoveTo(target),
    parabolicMoveTo: (target, height) => height === void 0 ? lainPet.parabolicMoveTo(target) : lainPet.parabolicMoveTo(target, height),
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
      document.addEventListener("DOMContentLoaded", pendingStart, { once: true });
    } else {
      start();
    }
  }
  install();
})();
