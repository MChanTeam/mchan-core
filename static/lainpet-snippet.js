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
      idle: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/1.png",
      right: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk1.gif",
      left: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk2.gif"
    },
    school: {
      idle: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/115.png",
      right: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk3.gif",
      left: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk4.gif",
      event: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainburn.gif"
    },
    pink: {
      idle: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/116.png",
      right: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk5.gif",
      left: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk6.gif",
      event: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/laindance.gif"
    },
    bear: {
      idle: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/117.png",
      right: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk7.gif",
      left: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk8.gif",
      event: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainroll.gif"
    },
    home: {
      idle: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/118.png",
      right: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk9.gif",
      left: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/lainwalk10.gif"
    },
    misc: {
      crow: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/crow.gif",
      girl: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/flyinggirl.gif",
      navi: [
        "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/navi1.gif",
        "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/navi2.gif",
        "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/navi3.gif"
      ],
      exp1: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/expression1.gif?raw=true",
      exp2: "https://raw.githubusercontent.com/realmxrza/Lain-Discord/main/src/expression2.gif?raw=true"
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

  // LainPet/classes/LainPet.ts
  var SPRITE_SIZE = {
    normal: 100,
    event: 200
  };
  var IDLE_DURATION = 5e3;
  var MOVEMENT_TIMEOUT = 1e4;
  var WALK_RADIUS = 500;
  var WALK_MIN_DISTANCE = 100;
  var TARGET_RADIUS = 5;
  var LainPet = class {
    constructor() {
      __publicField(this, "lainElements", null);
      __publicField(this, "physicsInterval", null);
      __publicField(this, "timeouts", new TimeoutRegistry());
      __publicField(this, "idleUntil", 0);
      __publicField(this, "movementTimeout", null);
      __publicField(this, "movementTarget", null);
      __publicField(this, "movementTimedOut", false);
      __publicField(this, "facing", "right");
      __publicField(this, "expressionTimeout", null);
      __publicField(this, "dialogueTimeout", null);
      __publicField(this, "sugarRushTimeout", null);
      __publicField(this, "eventTimeout", null);
      __publicField(this, "expressionInvocation", 0);
      __publicField(this, "dialogueInvocation", 0);
      __publicField(this, "sugarRushInvocation", 0);
      __publicField(this, "eventInvocation", 0);
      __publicField(this, "dragMoveHandler", null);
      __publicField(this, "dragUpHandler", null);
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
    }
    schedule(callback, delay) {
      return this.timeouts.schedule(callback, delay);
    }
    cancelTimeout(timeoutId) {
      this.timeouts.cancel(timeoutId);
    }
    removeDragListeners() {
      if (this.dragMoveHandler) {
        window.removeEventListener("mousemove", this.dragMoveHandler);
        this.dragMoveHandler = null;
      }
      if (this.dragUpHandler) {
        window.removeEventListener("mouseup", this.dragUpHandler);
        this.dragUpHandler = null;
      }
    }
    cancelMovementTimeout() {
      if (this.movementTimeout === null) return;
      this.cancelTimeout(this.movementTimeout);
      this.movementTimeout = null;
    }
    startMovement(target, timeout = MOVEMENT_TIMEOUT) {
      const sameTarget = this.movementTarget?.x === target.x && this.movementTarget?.y === target.y;
      if (sameTarget && this.state.mode === "walk") {
        return true;
      }
      if (sameTarget && this.movementTimedOut && Date.now() < this.idleUntil) {
        return false;
      }
      this.cancelMovementTimeout();
      this.movementTarget = { ...target };
      this.movementTimedOut = false;
      this.state.target = { ...target };
      this.state.mode = "walk";
      const timeoutId = this.schedule(() => {
        if (this.movementTimeout !== timeoutId) return;
        this.movementTimeout = null;
        this.movementTimedOut = true;
        this.state.mode = "idle";
        this.state.velocity = { x: 0, y: 0 };
        this.idleUntil = Date.now() + IDLE_DURATION;
      }, timeout);
      this.movementTimeout = timeoutId;
      return true;
    }
    finishMovement() {
      this.cancelMovementTimeout();
      this.movementTarget = null;
      this.movementTimedOut = false;
      this.state.mode = "idle";
      this.state.velocity = { x: 0, y: 0 };
      this.idleUntil = Date.now() + IDLE_DURATION;
    }
    start() {
      if (this.lainElements) return;
      const lainState = this.state;
      const container = document.createElement("div");
      const lainSprite = document.createElement("img");
      const bubble = document.createElement("div");
      const expression = document.createElement("img");
      this.lainElements = {
        container,
        lainSprite,
        bubble,
        expression
      };
      container.style.cssText = `
	position:fixed;
	z-index:9999;
	pointer-events:none;
	top:0;
	left:0;
	width:100vw;
	height:100vh;
	`;
      lainSprite.style.cssText = `
	position:absolute;
	width:100px;
	pointer-events:auto;
	cursor:grab;
	transition: filter 0.2s;
	object-fit: contain;
	`;
      bubble.style.cssText = `
	position:absolute;
	background:white;
	color:black; border:2px solid black;
	padding:8px;
	border-radius:10px;
	font-family:monospace;
	font-size:12px; opacity:0;
	transition: opacity 0.5s;
	width:150px;
	text-align:center;
	z-index:10000;
	pointer-events:none;
	`;
      expression.style.cssText = `
	position:absolute;
	width:50px;
	opacity:0;
	transition: opacity 0.3s;
	z-index:10001;
	pointer-events:none;
	`;
      lainSprite.onmousedown = (event) => {
        event.preventDefault();
        lainState.isDragging = true;
        this.removeDragListeners();
        const dragMoveHandler = (ev) => {
          lainState.position.x = ev.clientX - 50;
          lainState.position.y = ev.clientY - 50;
          this.draw();
        };
        const dragUpHandler = () => {
          lainState.isDragging = false;
          this.removeDragListeners();
        };
        this.dragMoveHandler = dragMoveHandler;
        this.dragUpHandler = dragUpHandler;
        window.addEventListener("mousemove", dragMoveHandler);
        window.addEventListener("mouseup", dragUpHandler);
      };
      document.body.appendChild(container);
      container.appendChild(lainSprite);
      container.appendChild(bubble);
      container.appendChild(expression);
      this.physicsInterval = window.setInterval(() => {
        this.updatePhysics();
      }, 30);
    }
    stop() {
      const lainState = this.state;
      const lainElements = this.lainElements;
      if (this.physicsInterval !== null) {
        window.clearInterval(this.physicsInterval);
        this.physicsInterval = null;
      }
      this.cancelMovementTimeout();
      this.timeouts.clear();
      this.expressionTimeout = null;
      this.dialogueTimeout = null;
      this.sugarRushTimeout = null;
      this.eventTimeout = null;
      this.expressionInvocation++;
      this.dialogueInvocation++;
      this.sugarRushInvocation++;
      this.eventInvocation++;
      this.removeDragListeners();
      if (lainElements) {
        lainElements.lainSprite.onmousedown = null;
        lainElements.container.remove();
        this.lainElements = null;
      }
      lainState.position = { x: 100, y: 100 };
      lainState.velocity = { x: 0, y: 0 };
      lainState.target = { x: 100, y: 100 };
      lainState.mode = "idle";
      lainState.isDragging = false;
      lainState.eventActive = false;
      lainState.sugarRush = false;
      this.idleUntil = 0;
      this.movementTarget = null;
      this.movementTimedOut = false;
      this.facing = "right";
    }
    isRunning() {
      return this.lainElements !== null;
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
      if (!this.isRunning()) return null;
      return { ...this.state.position };
    }
    isWithinTargetRadius(target) {
      return magnitude(subtract(target, this.state.position)) <= TARGET_RADIUS;
    }
    moveTo(target) {
      if (!this.isRunning() || this.state.sugarRush) return;
      this.state.target = { ...target };
      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
        return;
      }
      this.startMovement(target);
    }
    draw() {
      if (!this.lainElements) return;
      const lainState = this.state;
      const lainSpriteStyle = this.lainElements?.lainSprite.style;
      const lainBubbleStyle = this.lainElements?.bubble.style;
      const lainExpressionStyle = this.lainElements?.expression.style;
      const size = this.state.eventActive ? SPRITE_SIZE.event : SPRITE_SIZE.normal;
      lainSpriteStyle.width = `${size}px`;
      lainSpriteStyle.left = `${lainState.position.x}px`;
      lainSpriteStyle.top = `${lainState.position.y}px`;
      lainBubbleStyle.left = `${lainState.position.x + size / 2 - 75}px`;
      lainBubbleStyle.top = `${lainState.position.y - 50}px`;
      lainExpressionStyle.left = `${lainState.position.x + size / 2 - 25}px`;
      lainExpressionStyle.top = `${lainState.position.y - 40}px`;
      if (!lainState.eventActive) {
        const outfitAssets = assets[lainState.outfit];
        let spriteUrl = outfitAssets.idle;
        if (lainState.mode === "walk") {
          spriteUrl = this.facing === "right" ? outfitAssets.right : outfitAssets.left;
        }
        if (this.lainElements.lainSprite.src !== spriteUrl) {
          this.lainElements.lainSprite.src = spriteUrl;
        }
      }
      if (lainState.sugarRush) {
        const hue = Date.now() % 360;
        lainSpriteStyle.filter = `hue-rotate(${hue}deg) brightness(1.2)`;
      } else {
        lainSpriteStyle.filter = "";
      }
    }
    updatePhysics() {
      const lainState = this.state;
      if (lainState.isDragging) return;
      if (lainState.eventActive) {
        this.moveToEventCenter();
      } else if (lainState.sugarRush) {
        this.moveDuringSugarRush();
      } else if (lainState.mode == "walk") {
        this.moveTowardTarget();
      } else {
        this.tryToStartWalking();
      }
      this.draw();
    }
    moveToEventCenter() {
      const size = SPRITE_SIZE.event;
      const center = {
        x: (window.innerWidth - size) / 2,
        y: (window.innerHeight - size) / 2
      };
      if (this.isWithinTargetRadius(center)) {
        this.finishMovement();
        return;
      }
      if (!this.startMovement(center)) return;
      this.moveToward(center);
    }
    moveToward(target) {
      const lainState = this.state;
      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
        return;
      }
      const displacement = subtract(target, lainState.position);
      const distance = magnitude(displacement);
      const step = scale(
        scale(displacement, 1 / distance),
        Math.min(lainState.speed, distance - TARGET_RADIUS)
      );
      lainState.velocity = step;
      lainState.position = add(lainState.position, step);
      if (step.x < 0) {
        this.facing = "left";
      } else if (step.x > 0) {
        this.facing = "right";
      }
      if (this.isWithinTargetRadius(target)) {
        this.finishMovement();
      }
    }
    moveTowardTarget() {
      this.moveToward(this.state.target);
    }
    moveDuringSugarRush() {
      const lainState = this.state;
      const maxX = Math.max(0, window.innerWidth - SPRITE_SIZE.normal);
      const maxY = Math.max(0, window.innerHeight - SPRITE_SIZE.normal);
      lainState.position = add(
        lainState.position,
        scale(lainState.velocity, 1.3)
      );
      if (lainState.position.x <= 0 || lainState.position.x >= maxX) {
        lainState.velocity.x *= -1;
      }
      if (lainState.position.y <= 0 || lainState.position.y >= maxY) {
        lainState.velocity.y *= -1;
      }
      lainState.position.x = Math.max(0, Math.min(lainState.position.x, maxX));
      lainState.position.y = Math.max(0, Math.min(lainState.position.y, maxY));
    }
    tryToStartWalking() {
      const lainState = this.state;
      if (Date.now() < this.idleUntil) return;
      if (Math.random() >= 0.01) return;
      const angle = Math.random() * Math.PI * 2;
      const distance = WALK_MIN_DISTANCE + Math.random() * (WALK_RADIUS - WALK_MIN_DISTANCE);
      const maxX = Math.max(0, window.innerWidth - SPRITE_SIZE.normal);
      const maxY = Math.max(0, window.innerHeight - SPRITE_SIZE.normal);
      const unclampedTarget = {
        x: lainState.position.x + Math.cos(angle) * distance,
        y: lainState.position.y + Math.sin(angle) * distance
      };
      const target = {
        x: Math.max(0, Math.min(unclampedTarget.x, maxX)),
        y: Math.max(0, Math.min(unclampedTarget.y, maxY))
      };
      const targetDistance = magnitude(subtract(target, lainState.position));
      if (targetDistance <= TARGET_RADIUS || targetDistance > WALK_RADIUS) {
        return;
      }
      this.startMovement(target);
    }
    triggerExpression() {
      const lainState = this.state;
      const lainElements = this.lainElements;
      if (!lainElements || lainState.eventActive) return;
      const expressionUrl = lainState.outfit === "bear" ? assets.misc.exp2 : assets.misc.exp1;
      lainElements.expression.src = expressionUrl;
      this.showTemporarily(lainElements.expression, 3e3);
    }
    triggerSpecialEvent(outfit = this.state.outfit) {
      const lainState = this.state;
      const lainElements = this.lainElements;
      if (!lainElements || lainState.eventActive || lainState.sugarRush) return;
      const eventAsset = assets[outfit]?.event;
      if (!eventAsset) return;
      const eventDurations = {
        bear: 8e3,
        school: 3e3,
        pink: 1e4
      };
      const duration = eventDurations[outfit] ?? 1e4;
      const invocation = ++this.eventInvocation;
      this.cancelTimeout(this.eventTimeout);
      lainState.eventActive = true;
      lainElements.lainSprite.src = eventAsset;
      this.draw();
      const timeoutId = this.schedule(() => {
        if (this.eventInvocation !== invocation) return;
        this.eventTimeout = null;
        lainState.eventActive = false;
        this.finishMovement();
        this.draw();
      }, duration);
      this.eventTimeout = timeoutId;
    }
    showTemporarily(lainElements, duration) {
      const invocation = ++this.expressionInvocation;
      this.cancelTimeout(this.expressionTimeout);
      lainElements.style.opacity = "1";
      const timeoutId = this.schedule(() => {
        if (this.expressionInvocation !== invocation) return;
        this.expressionTimeout = null;
        lainElements.style.opacity = "0";
      }, duration);
      this.expressionTimeout = timeoutId;
    }
    triggerSugarRush() {
      const lainState = this.state;
      if (lainState.eventActive) return;
      const randomSign = () => Math.random() > 0.5 ? 1 : -1;
      const invocation = ++this.sugarRushInvocation;
      this.cancelTimeout(this.sugarRushTimeout);
      this.cancelMovementTimeout();
      this.movementTarget = null;
      this.movementTimedOut = false;
      lainState.mode = "idle";
      lainState.target = { ...lainState.position };
      lainState.sugarRush = true;
      lainState.velocity = scale(
        {
          x: randomSign(),
          y: randomSign()
        },
        10
      );
      const timeoutId = this.schedule(() => {
        if (this.sugarRushInvocation !== invocation) return;
        this.sugarRushTimeout = null;
        lainState.sugarRush = false;
        this.finishMovement();
      }, 5e3);
      this.sugarRushTimeout = timeoutId;
    }
    showDialogue(text) {
      const lainElements = this.lainElements;
      if (!lainElements) return;
      const message = text ?? dialogues[Math.floor(Math.random() * dialogues.length)];
      const invocation = ++this.dialogueInvocation;
      this.cancelTimeout(this.dialogueTimeout);
      lainElements.bubble.textContent = message;
      lainElements.bubble.style.opacity = "1";
      const timeoutId = this.schedule(() => {
        if (this.dialogueInvocation !== invocation) return;
        this.dialogueTimeout = null;
        lainElements.bubble.style.opacity = "0";
      }, 4e3);
      this.dialogueTimeout = timeoutId;
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
  var lainPet = new LainPet();
  var navi = new Navi(assets.misc.navi);
  var flyingMisc = new FlyingMisc({
    crow: assets.misc.crow,
    girl: assets.misc.girl
  });
  var ambientInterval = null;
  var outfitInterval = null;
  var naviInterval = null;
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
    lainPet.start();
    startAmbientBehavior();
    console.log(
      "%c Lain Pet by realmxrza | standalone browser snippet started ",
      "background: #000; color: #f0f; font-weight: bold; font-size: 14px;"
    );
  }
  function stop() {
    stopAmbientBehavior();
    navi.stop();
    flyingMisc.stop();
    lainPet.stop();
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
    getPosition: () => lainPet.getPosition()
  };
  function install() {
    window.LainPet?.stop();
    window.LainPet = api;
    window.Lain = api;
    if (document.readyState === "loading") {
      window.addEventListener("DOMContentLoaded", start, { once: true });
    } else {
      start();
    }
  }
  install();
})();
