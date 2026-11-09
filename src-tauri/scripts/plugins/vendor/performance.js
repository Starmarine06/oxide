// ---- vendored from Pear Desktop: rm3/rm3.js (MIT) ----
/*

MIT License

Copyright 2024-2025 CY Fung

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/

/* oxlint-disable */

const rm3 = {};

const injectRm3 = () => {
  const DEBUG_OPT = false;
  const CONFIRM_TIME = 4000;
  const CHECK_INTERVAL = 400;
  const DEBUG_dataChangeReflection = true;

  /** @type {globalThis.PromiseConstructor} */
  const Promise = (async () => {})().constructor; // hacks Promise in WaterFox Classic and "Promise.resolve(0)" nevers resolve.

  // https://qiita.com/piroor/items/02885998c9f76f45bfa0
  // https://gist.github.com/piroor/829ecb32a52c2a42e5393bbeebe5e63f
  function uniq(array) {
    return [...new Set(array)];
  }

  rm3.uniq = uniq; // [[debug]]
  DEBUG_OPT && (rm3.location = location.href);

  rm3.inspect = () => {
    return uniq(
      [...document.getElementsByTagName('*')]
        .filter((e) => e?.polymerController?.createComponent_)
        .map((e) => e.nodeName),
    ); // [[debug]]
  };

  const insp = (o) => o ? o.polymerController || o.inst || o || 0 : o || 0;
  const indr = (o) => insp(o).$ || o.$ || 0;

  const getProto = (element) => {
    if (element) {
      const cnt = insp(element);
      return cnt.constructor.prototype || null;
    }
    return null;
  };

  const LinkedArray = (() => {
    class Node {
      constructor(value) {
        this.value = value;
        this.next = null;
        this.prev = null;
      }
    }

    class LinkedArray {
      constructor() {
        this.head = null;
        this.tail = null;
        this.length = 0;
      }

      push(value) {
        const newNode = new Node(value);
        if (this.length === 0) {
          this.head = newNode;
          this.tail = newNode;
        } else {
          this.tail.next = newNode;
          newNode.prev = this.tail;
          this.tail = newNode;
        }
        this.length++;
        return this.length;
      }

      pop() {
        if (this.length === 0) return undefined;
        const removedNode = this.tail;
        if (this.length === 1) {
          this.head = null;
          this.tail = null;
        } else {
          this.tail = removedNode.prev;
          this.tail.next = null;
          removedNode.prev = null;
        }
        this.length--;
        return removedNode.value;
      }

      unshift(value) {
        const newNode = new Node(value);
        if (this.length === 0) {
          this.head = newNode;
          this.tail = newNode;
        } else {
          newNode.next = this.head;
          this.head.prev = newNode;
          this.head = newNode;
        }
        this.length++;
        return this.length;
      }

      shift() {
        if (this.length === 0) return undefined;
        const removedNode = this.head;
        if (this.length === 1) {
          this.head = null;
          this.tail = null;
        } else {
          this.head = removedNode.next;
          this.head.prev = null;
          removedNode.next = null;
        }
        this.length--;
        return removedNode.value;
      }

      size() {
        return this.length;
      }

      // Get a node by index (0-based)
      getNode(index) {
        if (index < 0 || index >= this.length) return null;

        let current;
        let counter;

        // Optimization: start from closest end
        if (index < this.length / 2) {
          current = this.head;
          counter = 0;
          while (counter !== index) {
            current = current.next;
            counter++;
          }
        } else {
          current = this.tail;
          counter = this.length - 1;
          while (counter !== index) {
            current = current.prev;
            counter--;
          }
        }
        return current;
      }

      // Get value by index
      get(index) {
        const node = this.getNode(index);
        return node ? node.value : undefined;
      }

      // Find the first node with the given value and return both node and index
      findNode(value) {
        let current = this.head;
        let idx = 0;
        while (current) {
          if (current.value === value) {
            return { node: current, index: idx };
          }
          current = current.next;
          idx++;
        }
        return { node: null, index: -1 };
      }

      toArray() {
        const arr = [];
        let current = this.head;
        while (current) {
          arr.push(current.value);
          current = current.next;
        }
        return arr;
      }

      // Insert a new value before a given node (provided you already have the node reference)
      insertBeforeNode(node, newValue) {
        if (!node) {
          this.unshift(newValue);
          return true;
        }

        if (node === this.head) {
          // If the target is the head, just unshift
          this.unshift(newValue);
        } else {
          const newNode = new Node(newValue);
          const prevNode = node.prev;

          prevNode.next = newNode;
          newNode.prev = prevNode;
          newNode.next = node;
          node.prev = newNode;

          this.length++;
        }
        return true;
      }

      // Insert a new value after a given node (provided you already have the node reference)
      insertAfterNode(node, newValue) {
        if (!node) {
          this.push(newValue);
          return true;
        }

        if (node === this.tail) {
          // If the target is the tail, just push
          this.push(newValue);
        } else {
          const newNode = new Node(newValue);
          const nextNode = node.next;

          node.next = newNode;
          newNode.prev = node;
          newNode.next = nextNode;
          nextNode.prev = newNode;

          this.length++;
        }
        return true;
      }

      // Insert a new value before the first occurrence of an existing value (search by value)
      insertBefore(existingValue, newValue) {
        const { node } = this.findNode(existingValue);
        if (!node) return false; // Not found
        return this.insertBeforeNode(node, newValue);
      }

      // Insert a new value after the first occurrence of an existing value (search by value)
      insertAfter(existingValue, newValue) {
        const { node } = this.findNode(existingValue);
        if (!node) return false; // Not found
        return this.insertAfterNode(node, newValue);
      }

      // Delete a given node from the list
      deleteNode(node) {
        if (!node) return false;

        if (this.length === 1 && node === this.head && node === this.tail) {
          // Only one element in the list
          this.head = null;
          this.tail = null;
        } else if (node === this.head) {
          // Node is the head
          this.head = node.next;
          this.head.prev = null;
          node.next = null;
        } else if (node === this.tail) {
          // Node is the tail
          this.tail = node.prev;
          this.tail.next = null;
          node.prev = null;
        } else {
          // Node is in the middle
          const prevNode = node.prev;
          const nextNode = node.next;
          prevNode.next = nextNode;
          nextNode.prev = prevNode;
          node.prev = null;
          node.next = null;
        }

        this.length--;
        return true;
      }
    }

    LinkedArray.Node = Node;
    return LinkedArray;
  })();

  class LimitedSizeSet extends Set {
    constructor(n) {
      super();
      this.limit = n;
    }

    add(key) {
      if (!super.has(key)) {
        super.add(key);
        let n = super.size - this.limit;
        if (n > 0) {
          const iterator = super.values();
          do {
            const firstKey = iterator.next().value; // Get the first (oldest) key
            super.delete(firstKey); // Delete the oldest key
          } while (--n > 0);
        }
      }
    }

    removeAdd(key) {
      super.delete(key);
      this.add(key);
    }
  }

  if (
    !document.createElement9512 &&
    typeof document.createElement === 'function' &&
    document.createElement.length === 1
  ) {
    // sizing of Map / Set. Shall limit ?

    const hookTos = new Set(); // [[debug]]
    DEBUG_OPT && (rm3.hookTos = hookTos);

    // const reusePool = new Map(); // xx858
    const entryRecords = new WeakMap(); // a weak link between element and record

    // rm3.list = [];

    const operations = rm3.operations = new Set(); // to find out the "oldest elements"

    const availablePools = rm3.availablePools = new Map(); // those "old elements" can be used
    let lastTimeCheck = 0;

    const reuseRecord_ = new LimitedSizeSet(256); // [[debug]]
    const reuseCount_ = new Map();

    let noTimeCheck = false;

    // const defaultValues = new Map();
    // const noValues = new Map();

    const timeCheck = () => {
      // regularly check elements are old enough to put into the available pools
      // note: the characterists of original components are non-volatile. So don't need to waste time to check weakRef.deref() is null or not for removing in operations.

      const ct = Date.now();
      if (ct - lastTimeCheck < CHECK_INTERVAL || noTimeCheck) return;
      lastTimeCheck = ct;
      noTimeCheck = true;

      // 16,777,216
      if (hookTos.size > 777216) hookTos.clear(); // just debug usage, dont concern
      if (operations.size > 7777216) {
        // extremely old elements in operations mean they have no attach/detach action. so no reuse as well. they are just trash in memory.
        // as no checking of the weakRef.deref() being null or not, those trash could be already cleaned. However we don't concern this.
        // (not to count whether they are actual memory trash or not)
        const half = operations.size >>> 1;
        let i = 0;
        for (const value of operations) {
          if (i++ > half) break;
          operations.delete(value);
        }
      }

      // {
      //   // smallest to largest
      //   // past to recent

      //   const iterator = operations[Symbol.iterator]();
      //   console.log(1831, '------------------------')
      //   while (true) {
      //     const iteratorResult = iterator.next(); // 順番に値を取りだす
      //     if (iteratorResult.done) break; // 取り出し終えたなら、break
      //     console.log(1835, iteratorResult.value[3])
      //   }

      //   console.log(1839, '------------------------')
      // }

      // Set iterator
      // s.add(2) s.add(6) s.add(1) s.add(3)
      // next: 2 -> 6 -> 1 -> 3
      // op1 (oldest) -> op2 -> op3 -> op4 (latest)
      const iterator = operations[Symbol.iterator]();

      const targetTime = ct - CONFIRM_TIME;

      const pivotNodes = new WeakMap();

      while (true) {
        const iteratorResult = iterator.next(); // 順番に値を取りだす
        if (iteratorResult.done) break; // 取り出し終えたなら、break
        const entryRecord = iteratorResult.value;
        if (entryRecord[3] > targetTime) break;

        if (!entryRecord[4] && entryRecord[1] < 0 && entryRecord[2] > 0) {
          const element = entryRecord[0].deref();
          const eKey = (element || 0).__rm3Tag003__;
          if (!eKey) {
            operations.delete(entryRecord);
          } else if (
            element.isConnected === false &&
            insp(element).isAttached === false
          ) {
            entryRecord[4] = true;

            let availablePool = availablePools.get(eKey);
            if (!availablePool)
              availablePools.set(eKey, availablePool = new LinkedArray());
            if (!(availablePool instanceof LinkedArray)) throw new Error();
            DEBUG_OPT &&
              console.log(3885, 'add key', eKey, availablePools.size);
            // rm3.showSize = ()=>availablePools.size
            // setTimeout(()=>{
            //   // window?.euu1 = availablePools
            //   // window?.euu2 = availablePools.size
            //   console.log(availablePools.size)
            // }, 8000)
            let pivotNode = pivotNodes.get(availablePool);
            if (!pivotNode)
              pivotNodes.set(availablePool, pivotNode = availablePool.head); // cached the previous newest node (head) as pivotNode

            availablePool.insertBeforeNode(pivotNode, entryRecord); // head = newest, tail = oldest
          }
        }
      }
      noTimeCheck = false;
    };

    const attachedDefine = function () {
      Promise.resolve().then(timeCheck);
      try {
        const hostElement = this?.hostElement;
        if (hostElement instanceof HTMLElement) {
          const entryRecord = entryRecords.get(hostElement);
          if (
            entryRecord &&
            entryRecord[0].deref() === hostElement &&
            hostElement.isConnected === true &&
            this?.isAttached === true
          ) {
            noTimeCheck = true;
            const ct = Date.now();
            entryRecord[1] = ct;
            entryRecord[2] = -1;
            entryRecord[3] = ct;
            operations.delete(entryRecord);
            operations.add(entryRecord);
            noTimeCheck = false;
            // note: because of performance prespective, deletion for availablePools[eKey]'s linked element would not be done here.
            // entryRecord[4] is not required to be updated here.
          }
        }
      } catch (e) {}
      return this.attached9512();
    };
    const detachedDefine = function () {
      Promise.resolve().then(timeCheck);
      try {
        const hostElement = this?.hostElement;
        if (hostElement instanceof HTMLElement) {
          const entryRecord = entryRecords.get(hostElement);
          if (
            entryRecord &&
            entryRecord[0].deref() === hostElement &&
            hostElement.isConnected === false &&
            this?.isAttached === false
          ) {
            noTimeCheck = true;
            const ct = Date.now();
            entryRecord[2] = ct;
            entryRecord[1] = -1;
            entryRecord[3] = ct;
            operations.delete(entryRecord);
            operations.add(entryRecord);
            noTimeCheck = false;
            // note: because of performance prespective, deletion for availablePools[eKey]'s linked element would not be done here.
            // entryRecord[4] is not required to be updated here.
          }
        }
      } catch (e) {}

      return this.detached9512();
    };

    // function cpy(x) {
    //   if (!x) return x;
    //   try {
    //     if (typeof x === 'object' && typeof x.length ==='number' && typeof x.slice === 'function') {
    //       x = x.slice(0)
    //     } else if (typeof x === 'object' && !x.length) {
    //       x = JSON.parse(JSON.stringify(x));
    //     } else {
    //       return Object.assign({}, x);
    //     }
    //   } catch (e) { }
    //   return x;
    // }

    async function digestMessage(message) {
      const msgUint8 = new TextEncoder().encode(message); // (utf-8 の) Uint8Array にエンコードする
      const hashBuffer = await crypto.subtle.digest('SHA-256', msgUint8); // メッセージをハッシュする
      const hashArray = Array.from(new Uint8Array(hashBuffer)); // バッファーをバイト列に変換する
      const hashHex = hashArray
        .map((b) => b.toString(16).padStart(2, '0'))
        .join(''); // バイト列を 16 進文字列に変換する
      return hashHex.toUpperCase();
    }

    let onPageContainer = null;

    // onCreateComponentError - see https://github.com/cyfung1031/userscript-supports/issues/99
    const createComponentDefine_ = function (a, b, c, onCreateComponentError) {
      Promise.resolve().then(timeCheck);

      const creatorTag = this?.is || this?.nodeName?.toLowerCase() || '';

      const componentTag = typeof a === 'string' ? a : (a || 0).component || '';

      const eKey =
        creatorTag && componentTag ? `${creatorTag}.${componentTag}` : '*'; // '*' for play-safe
      const availablePool = availablePools.get(eKey);

      try {
        if (availablePool instanceof LinkedArray) {
          noTimeCheck = true;

          let node = availablePool.tail; // oldest

          while (node instanceof LinkedArray.Node) {
            const entryRecord = node.value;
            const prevNode = node.prev;

            let ok = false;
            let elm = null;
            if (entryRecord[1] < 0 && entryRecord[2] > 0 && entryRecord[4]) {
              elm = entryRecord[0].deref();
              // elm && console.log(3882, (elm.__shady_native_textContent || elm.textContent))
              if (
                elm &&
                elm instanceof HTMLElement &&
                elm.isConnected === false &&
                insp(elm).isAttached === false &&
                elm.parentNode === null
              ) {
                ok = true;
              }
            }

            if (ok) {
              // useEntryRecord = entryRecord;
              entryRecord[4] = false;
              // console.log('nodeDeleted', 1, entryRecord[0].deref().nodeName)
              availablePool.deleteNode(node);
              // break;

              if (!onPageContainer) {
                let p = document.createElement('noscript');
                document.body.prepend(p);
                onPageContainer = p;
              }

              onPageContainer.appendChild(elm); // to fix some issues for the rendered elements

              const cnt = insp(elm);

              cnt.__dataInvalid = false;
              // cnt._initializeProtoProperties(cnt.data)

              // window.meaa = cnt.$.container;
              if (typeof (cnt.__data || 0) === 'object') {
                cnt.__data = Object.assign({}, cnt.__data);
              }
              cnt.__dataPending = {};
              cnt.__dataOld = {};

              try {
                cnt.markDirty();
              } catch (e) {}
              try {
                cnt.markDirtyVisibilityObserver();
              } catch (e) {}

              try {
                cnt.wasPrescan = cnt.wasVisible = !1;
              } catch (e) {}

              // try{
              //   cnt._setPendingProperty('data', Object.assign({}, cntData), !0);
              // }catch(e){}
              // try {
              //   cnt._flushProperties();
              // } catch (e) { }

              if (DEBUG_OPT && DEBUG_dataChangeReflection) {
                let jC1 = null;
                let jC2 = null;
                const jKey = `${Math.floor(Math.random() * 314159265359 + 314159265359).toString(36)}`;
                try {
                  jC1 =
                    cnt.hostElement.__shady_native_textContent ||
                    cnt.hostElement.textContent;
                  // console.log(83802, jKey, (cnt.hostElement.__shady_native_textContent || cnt.hostElement.textContent))
                } catch (e) {
                  console.warn(e);
                }

                setTimeout(() => {
                  try {
                    jC2 =
                      cnt.hostElement.__shady_native_textContent ||
                      cnt.hostElement.textContent;
                    // console.log(83804, jKey, (cnt.hostElement.__shady_native_textContent || cnt.hostElement.textContent))
                  } catch (e) {
                    console.warn(e);
                  }

                  (async () => {
                    jC1 = await digestMessage(jC1);
                    jC2 = await digestMessage(jC2);

                    console.log(
                      83804,
                      jKey,
                      jC1.substring(0, 7),
                      jC2.substring(0, 7),
                    );
                  })();
                }, 1000);
              }

              if (entryRecord[5] < 1e9) entryRecord[5] += 1;
              DEBUG_OPT &&
                Promise.resolve().then(() =>
                  console.log(`${eKey} reuse`, entryRecord),
                ); // give some time for attach process
              DEBUG_OPT && reuseRecord_.add([Date.now(), cnt.is, entryRecord]);
              DEBUG_OPT &&
                reuseCount_.set(cnt.is, (reuseCount_.get(cnt.is) || 0) + 1);
              if (rm3.reuseCount < 1e9) rm3.reuseCount++;

              return elm;
            }

            // console.log('condi88', entryRecord[1] < 0 , entryRecord[2] > 0 , !!entryRecord[4], !!entryRecord[0].deref())

            entryRecord[4] = false;

            // console.log(entryRecord);
            // console.log('nodeDeleted',2, entryRecord[0]?.deref()?.nodeName)
            availablePool.deleteNode(node);
            node = prevNode;
          }
          // for(const ) availablePool
          // noTimeCheck = false;
        }
      } catch (e) {
        console.warn(e);
      }
      noTimeCheck = false;

      // console.log('createComponentDefine_', a, b, c)

      // if (!reusePool.has(componentTag)) reusePool.set(componentTag, new LinkedArray()); // xx858

      // const pool = reusePool.get(componentTag); // xx858
      // if (!(pool instanceof LinkedArray)) throw new Error(); // xx858

      const newElement = this.createComponent9512_(a, b, c, onCreateComponentError);
      // if(componentTag.indexOf( 'ticker')>=0)console.log(1883, a,newElement)

      try {
        const cntE = insp(newElement);
        if (!cntE.attached9512 && cntE.attached) {
          const cProtoE = getProto(newElement);

          if (cProtoE.attached === cntE.attached) {
            if (
              !cProtoE.attached9512 &&
              typeof cProtoE.attached === 'function' &&
              cProtoE.attached.length === 0
            ) {
              cProtoE.attached9512 = cProtoE.attached;

              cProtoE.attached = attachedDefine;
              // hookTos.add(a);
            }
          } else {
            if (
              typeof cntE.attached === 'function' &&
              cntE.attached.length === 3
            ) {
              cntE.attached9512 = cntE.attached;

              cntE.attached = attachedDefine;
              // hookTos.add(a);
            }
          }
        }

        if (!cntE.detached9512 && cntE.detached) {
          const cProtoE = getProto(newElement);

          if (cProtoE.detached === cntE.detached) {
            if (
              !cProtoE.detached9512 &&
              typeof cProtoE.detached === 'function' &&
              cProtoE.detached.length === 0
            ) {
              cProtoE.detached9512 = cProtoE.detached;

              cProtoE.detached = detachedDefine;
              // hookTos.add(a);
            }
          } else {
            if (
              typeof cntE.detached === 'function' &&
              cntE.detached.length === 3
            ) {
              cntE.detached9512 = cntE.detached;

              cntE.detached = detachedDefine;
              // hookTos.add(a);
            }
          }
        }

        const acceptance = true;
        // const acceptance = !cntE.__dataReady && cntE.__dataInvalid !== false; // we might need to change the acceptance condition along with Coding updates.
        if (acceptance) {
          // [[ weak ElementNode, attached time, detached time, time of change, inside availablePool, reuse count ]]
          const entryRecord = [new WeakRef(newElement), -1, -1, -1, false, 0];

          newElement.__rm3Tag003__ = eKey;
          entryRecords.set(newElement, entryRecord);
        } else {
        }
      } catch (e) {
        console.warn(e);
      }
      return newElement;
    };

    document.createElement9512 = document.createElement;
    document.createElement = function (a) {
      const r = document.createElement9512(a);
      try {
        const cnt = insp(r);
        if (cnt.createComponent_ && !cnt.createComponent9512_) {
          const cProto = getProto(r);
          if (cProto.createComponent_ === cnt.createComponent_) {
            if (
              !cProto.createComponent9512_ &&
              typeof cProto.createComponent_ === 'function' &&
              (
                cProto.createComponent_.length === 4 ||
                cProto.createComponent_.length === 3
              )
            ) {
              cProto.createComponent9512_ = cProto.createComponent_;

              cProto.createComponent_ = createComponentDefine_;
              DEBUG_OPT && hookTos.add(a);
            }
          } else {
            if (
              typeof cnt.createComponent_ === 'function' &&
              (
                cnt.createComponent_.length === 4 ||
                cnt.createComponent_.length === 3
              )
            ) {
              cnt.createComponent9512_ = cnt.createComponent_;

              cnt.createComponent_ = createComponentDefine_;
              DEBUG_OPT && hookTos.add(a);
            }
          }
        }
      } catch (e) {
        console.warn(e);
      }

      return r;
    };

    rm3.checkWhetherUnderParent = () => {
      const r = [];
      for (const operation of operations) {
        const elm = operation[0].deref();
        if (operation[2] > 0) {
          r.push([
            !!elm,
            elm?.nodeName.toLowerCase(),
            elm?.parentNode === null,
          ]);
        }
      }
      return r;
    };

    rm3.hookTags = () => {
      const r = new Set();
      for (const operation of operations) {
        const elm = operation[0].deref();
        if (elm && elm.is) {
          r.add(elm.is);
        }
      }
      return [...r];
    };

    DEBUG_OPT &&
      (rm3.reuseRecord = () => {
        return [...reuseRecord_]; // [[debug]]
      });

    DEBUG_OPT && (rm3.reuseCount_ = reuseCount_);
  }

  rm3.reuseCount = 0;
};

// ---- vendored from Pear Desktop: cpu-tamer/cpu-tamer-by-animationframe.js (MIT) ----
/*

MIT License

Copyright 2021-2025 CY Fung

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/

/* oxlint-disable */

const injectCpuTamerByAnimationFrame = ((__CONTEXT__) => {
  'use strict';

  const win = this instanceof Window ? this : window;

  // Create a unique key for the script and check if it is already running
  const hkey_script = 'nzsxclvflluv';
  if (win[hkey_script]) throw new Error('Duplicated Userscript Calling'); // avoid duplicated scripting
  win[hkey_script] = true;

  /** @type {globalThis.PromiseConstructor} */
  const Promise = (async () => { })().constructor; // hacks Promise in WaterFox Classic and "Promise.resolve(0)" nevers resolve.
  const PromiseExternal = ((resolve_, reject_) => {
    const h = (resolve, reject) => { resolve_ = resolve; reject_ = reject };
    return class PromiseExternal extends Promise {
      constructor(cb = h) {
        super(cb);
        if (cb === h) {
          /** @type {(value: any) => void} */
          this.resolve = resolve_;
          /** @type {(reason?: any) => void} */
          this.reject = reject_;
        }
      }
    };
  })();

  const isGPUAccelerationAvailable = (() => {
    // https://gist.github.com/cvan/042b2448fcecefafbb6a91469484cdf8
    try {
      const canvas = document.createElement('canvas');
      return !!(canvas.getContext('webgl') || canvas.getContext('experimental-webgl'));
    } catch (e) {
      return false;
    }
  })();

  if (!isGPUAccelerationAvailable) {
    throw new Error('Your browser does not support GPU Acceleration. CPU Tamer by AnimationFrame is skipped.');
  }

  const timeupdateDT = (() => {

    window.__j6YiAc__ = 1;

    document.addEventListener('timeupdate', () => {
      window.__j6YiAc__ = Date.now();
    }, true);

    let kz = -1;
    try {
      kz = top.__j6YiAc__;
    } catch (e) {

    }

    return kz >= 1 ? () => top.__j6YiAc__ : () => window.__j6YiAc__;

  })();

  const cleanContext = async (win) => {
    const waitFn = requestAnimationFrame; // shall have been binded to window
    try {
      let mx = 16; // MAX TRIAL
      const frameId = 'vanillajs-iframe-v1'
      let frame = document.getElementById(frameId);
      let removeIframeFn = null;
      if (!frame) {
        frame = document.createElement('iframe');
        frame.id = frameId;
        const blobURL = typeof webkitCancelAnimationFrame === 'function' && typeof kagi === 'undefined' ? (frame.src = URL.createObjectURL(new Blob([], { type: 'text/html' }))) : null; // avoid Brave Crash
        frame.sandbox = 'allow-same-origin'; // script cannot be run inside iframe but API can be obtained from iframe
        let n = document.createElement('noscript'); // wrap into NOSCRPIT to avoid reflow (layouting)
        n.appendChild(frame);
        while (!document.documentElement && mx-- > 0) await new Promise(waitFn); // requestAnimationFrame here could get modified by the engine
        const root = document.documentElement;
        root.appendChild(n); // throw error if root is null due to exceeding MAX TRIAL
        if (blobURL) Promise.resolve().then(() => URL.revokeObjectURL(blobURL));

        removeIframeFn = (setTimeout) => {
          const removeIframeOnDocumentReady = (e) => {
            e && win.removeEventListener("DOMContentLoaded", removeIframeOnDocumentReady, false);
            e = n;
            n = win = removeIframeFn = 0;
            setTimeout ? setTimeout(() => e.remove(), 200) : e.remove();
          }
          if (!setTimeout || document.readyState !== 'loading') {
            removeIframeOnDocumentReady();
          } else {
            win.addEventListener("DOMContentLoaded", removeIframeOnDocumentReady, false);
          }
        }
      }
      while (!frame.contentWindow && mx-- > 0) await new Promise(waitFn);
      const fc = frame.contentWindow;
      if (!fc) throw "window is not found."; // throw error if root is null due to exceeding MAX TRIAL
      try {
        const { requestAnimationFrame, setInterval, setTimeout, clearInterval, clearTimeout } = fc;
        const res = { requestAnimationFrame, setInterval, setTimeout, clearInterval, clearTimeout };
        for (let k in res) res[k] = res[k].bind(win); // necessary
        if (removeIframeFn) Promise.resolve(res.setTimeout).then(removeIframeFn);
        return res;
      } catch (e) {
        if (removeIframeFn) removeIframeFn();
        return null;
      }
    } catch (e) {
      console.warn(e);
      return null;
    }
  };

  cleanContext(win).then(__CONTEXT__ => {

    if (!__CONTEXT__) return null;

    const { requestAnimationFrame, setTimeout, setInterval, clearTimeout, clearInterval } = __CONTEXT__;

    /** @type {Function|null} */
    let afInterupter = null;

    const getRAFHelper = () => {
      const asc = document.createElement('a-f');
      if (!('onanimationiteration' in asc)) {
        return (resolve) => requestAnimationFrame(afInterupter = resolve);
      }
      asc.id = 'a-f';
      let qr = null;
      asc.onanimationiteration = function () {
        if (qr !== null) qr = (qr(), null);
      }
      if (!document.getElementById('afscript')) {
        const style = document.createElement('style');
        style.id = 'afscript';
        style.textContent = `
          @keyFrames aF1 {
            0% {
              order: 0;
            }
            100% {
              order: 1;
            }
          }
          #a-f[id] {
            visibility: collapse !important;
            position: fixed !important;
            display: block !important;
            top: -100px !important;
            left: -100px !important;
            margin:0 !important;
            padding:0 !important;
            outline:0 !important;
            border:0 !important;
            z-index:-1 !important;
            width: 0px !important;
            height: 0px !important;
            contain: strict !important;
            pointer-events: none !important;
            animation: 1ms steps(2, jump-none) 0ms infinite alternate forwards running aF1 !important;
          }
        `;
        (document.head || document.documentElement).appendChild(style);
      }
      document.documentElement.insertBefore(asc, document.documentElement.firstChild);
      return (resolve) => (qr = afInterupter = resolve);
    };

    /** @type {(resolve: () => void)}  */
    const rafPN = getRAFHelper(); // rAF will not execute if document is hidden

    (() => {
      let afPromiseP, afPromiseQ; // non-null
      afPromiseP = afPromiseQ = { resolved: true }; // initial state for !uP && !uQ
      let afix = 0;
      const afResolve = async (rX) => {
        await new Promise(rafPN);
        rX.resolved = true;
        const t = afix = (afix & 1073741823) + 1;
        return rX.resolve(t), t;
      };
      const eFunc = async () => {
        const uP = !afPromiseP.resolved ? afPromiseP : null;
        const uQ = !afPromiseQ.resolved ? afPromiseQ : null;
        let t = 0;
        if (uP && uQ) {
          const t1 = await uP;
          const t2 = await uQ;
          t = ((t1 - t2) & 536870912) === 0 ? t1 : t2; // = 0 for t1 - t2 = [0, 536870911], [–1073741824, -536870913]
        } else {
          const vP = !uP ? (afPromiseP = new PromiseExternal()) : null;
          const vQ = !uQ ? (afPromiseQ = new PromiseExternal()) : null;
          if (uQ) await uQ; else if (uP) await uP;
          if (vP) t = await afResolve(vP);
          if (vQ) t = await afResolve(vQ);
        }
        return t;
      }
      const inExec = new Set();
      const wFunc = async (handler, wStore) => {
        try {
          const ct = Date.now();
          if (ct - timeupdateDT() < 800 && ct - wStore.dt < 800) {
            const cid = wStore.cid;
            inExec.add(cid);
            const t = await eFunc();
            const didNotRemove = inExec.delete(cid); // true for valid key
            if (!didNotRemove || t === wStore.lastExecution) return;
            wStore.lastExecution = t;
          }
          wStore.dt = ct;
          handler();
        } catch (e) {
          console.error(e);
          throw e;
        }
      };
      const sFunc = (propFunc) => {
        return (func, ms = 0, ...args) => {
          if (typeof func === 'function') { // ignore all non-function parameter (e.g. string)
            const wStore = { dt: Date.now() };
            return (wStore.cid = propFunc(wFunc, ms, (args.length > 0 ? func.bind(null, ...args) : func), wStore));
          } else {
            return propFunc(func, ms, ...args);
          }
        };
      };
      win.setTimeout = sFunc(setTimeout);
      win.setInterval = sFunc(setInterval);

      const dFunc = (propFunc) => {
        return (cid) => {
          if (cid) inExec.delete(cid) || propFunc(cid);
        };
      };

      win.clearTimeout = dFunc(clearTimeout);
      win.clearInterval = dFunc(clearInterval);

      try {
        win.setTimeout.toString = setTimeout.toString.bind(setTimeout);
        win.setInterval.toString = setInterval.toString.bind(setInterval);
        win.clearTimeout.toString = clearTimeout.toString.bind(clearTimeout);
        win.clearInterval.toString = clearInterval.toString.bind(clearInterval);
      } catch (e) { console.warn(e) }

    })();

    let mInterupter = null;
    setInterval(() => {
      if (mInterupter === afInterupter) {
        if (mInterupter !== null) afInterupter = mInterupter = (mInterupter(), null);
      } else {
        mInterupter = afInterupter;
      }
    }, 125);
  });

});

// ---- vendored from Pear Desktop: cpu-tamer/cpu-tamer-by-dom-mutation.js (MIT) ----
/*

MIT License

Copyright 2024-2025 CY Fung

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/

/* oxlint-disable */

const injectCpuTamerByDomMutation = ((__CONTEXT__) => {
  'use strict';

  const win = this instanceof Window ? this : window;

  // Create a unique key for the script and check if it is already running
  const hkey_script = 'nzsxclvflluv';
  if (win[hkey_script]) throw new Error('Duplicated Userscript Calling'); // avoid duplicated scripting
  win[hkey_script] = true;

  /** @type {globalThis.PromiseConstructor} */
  const Promise = (async () => { })().constructor; // hacks Promise in WaterFox Classic and "Promise.resolve(0)" nevers resolve.
  const PromiseExternal = ((resolve_, reject_) => {
    const h = (resolve, reject) => { resolve_ = resolve; reject_ = reject };
    return class PromiseExternal extends Promise {
      constructor(cb = h) {
        super(cb);
        if (cb === h) {
          /** @type {(value: any) => void} */
          this.resolve = resolve_;
          /** @type {(reason?: any) => void} */
          this.reject = reject_;
        }
      }
    };
  })();

  // for future use
  /*
  const timeupdateDT = (() => {

    window.__j6YiAc__ = 1;

    document.addEventListener('timeupdate', () => {
      window.__j6YiAc__ = Date.now();
    }, true);

    let kz = -1;
    try {
      kz = top.__j6YiAc__;
    } catch (e) {

    }

    return kz >= 1 ? () => top.__j6YiAc__ : () => window.__j6YiAc__;

  })();
  */

  const cleanContext = async (win) => {
    const waitFn = requestAnimationFrame; // shall have been binded to window
    try {
      let mx = 16; // MAX TRIAL
      const frameId = 'vanillajs-iframe-v1'
      let frame = document.getElementById(frameId);
      let removeIframeFn = null;
      if (!frame) {
        frame = document.createElement('iframe');
        frame.id = frameId;
        const blobURL = typeof webkitCancelAnimationFrame === 'function' && typeof kagi === 'undefined' ? (frame.src = URL.createObjectURL(new Blob([], { type: 'text/html' }))) : null; // avoid Brave Crash
        frame.sandbox = 'allow-same-origin'; // script cannot be run inside iframe but API can be obtained from iframe
        let n = document.createElement('noscript'); // wrap into NOSCRPIT to avoid reflow (layouting)
        n.appendChild(frame);
        while (!document.documentElement && mx-- > 0) await new Promise(waitFn); // requestAnimationFrame here could get modified by the engine
        const root = document.documentElement;
        root.appendChild(n); // throw error if root is null due to exceeding MAX TRIAL
        if (blobURL) Promise.resolve().then(() => URL.revokeObjectURL(blobURL));

        removeIframeFn = (setTimeout) => {
          const removeIframeOnDocumentReady = (e) => {
            e && win.removeEventListener("DOMContentLoaded", removeIframeOnDocumentReady, false);
            e = n;
            n = win = removeIframeFn = 0;
            setTimeout ? setTimeout(() => e.remove(), 200) : e.remove();
          }
          if (!setTimeout || document.readyState !== 'loading') {
            removeIframeOnDocumentReady();
          } else {
            win.addEventListener("DOMContentLoaded", removeIframeOnDocumentReady, false);
          }
        }
      }
      while (!frame.contentWindow && mx-- > 0) await new Promise(waitFn);
      const fc = frame.contentWindow;
      if (!fc) throw "window is not found."; // throw error if root is null due to exceeding MAX TRIAL
      try {
        const { requestAnimationFrame, setInterval, setTimeout, clearInterval, clearTimeout } = fc;
        const res = { requestAnimationFrame, setInterval, setTimeout, clearInterval, clearTimeout };
        for (let k in res) res[k] = res[k].bind(win); // necessary
        if (removeIframeFn) Promise.resolve(res.setTimeout).then(removeIframeFn);
        return res;
      } catch (e) {
        if (removeIframeFn) removeIframeFn();
        return null;
      }
    } catch (e) {
      console.warn(e);
      return null;
    }
  };


  const { _setAttribute, _insertBefore, _hasAttribute } = (() => {
    let _setAttribute = Element.prototype.setAttribute;
    try {
      _setAttribute = ShadyDOM.nativeMethods.setAttribute || _setAttribute;
    } catch (e) { }
    let _hasAttribute = Element.prototype.hasAttribute;
    try {
      _hasAttribute = ShadyDOM.nativeMethods.hasAttribute || _hasAttribute;
    } catch (e) { }
    let _insertBefore = Node.prototype.insertBefore;
    try {
      _insertBefore = ShadyDOM.nativeMethods.insertBefore || _insertBefore;
    } catch (e) { }
    return { _setAttribute, _insertBefore, _hasAttribute};
  })();

  cleanContext(win).then(__CONTEXT__ => {

    if (!__CONTEXT__) return null;

    const { setTimeout, setInterval, clearTimeout, clearInterval } = __CONTEXT__;

    /*
    /-** @type {Function|null} *-/
    // let afInterupter = null;
    */

    const getDMHelper = () => {
      let _dm = document.getElementById('d-m');
      if (!_dm) {
        _dm = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
        _dm.id = 'd-m';
        _insertBefore.call(document.documentElement, _dm, document.documentElement.firstChild);
      }
      const dm = _dm;
      dm._setAttribute = _setAttribute;
      dm._hasAttribute = _hasAttribute;
      let j = 0;
      let attributeName_;
      while (dm._hasAttribute(attributeName_ = `dm-${Math.floor(Math.random() * 314159265359 + 314159265359).toString(36)}`)) {
        // none
      }
      const attributeName = attributeName_;
      let sr = null;
      const mo = new MutationObserver(() => {
        const sr_ = sr;
        if (sr_ !== null) {
          sr = null;
          if (j > 8) j = 0;
          sr_.resolve();
        }
      });
      mo.observe(document, { childList: true, subtree: true, attributes: true });
      return () => {
        return sr || (sr = (dm._setAttribute(attributeName, ++j), (new PromiseExternal()))); // mutationcallback in next macrotask
      };
    };

    /** @type {(resolve: () => void)}  */
    const dmSN = getDMHelper(); // dm will execute even if document is hidden

    (() => {
      let dmPromiseP, dmPromiseQ; // non-null
      dmPromiseP = dmPromiseQ = { resolved: true }; // initial state for !uP && !uQ
      let dmix = 0;
      const dmResolve = async (rX) => {
        await dmSN();
        rX.resolved = true;
        const t = dmix = (dmix & 1073741823) + 1;
        return rX.resolve(t), t;
      };
      const eFunc = async () => {
        const uP = !dmPromiseP.resolved ? dmPromiseP : null;
        const uQ = !dmPromiseQ.resolved ? dmPromiseQ : null;
        let t = 0;
        if (uP && uQ) {
          const t1 = await uP;
          const t2 = await uQ;
          t = ((t1 - t2) & 536870912) === 0 ? t1 : t2; // = 0 for t1 - t2 = [0, 536870911], [–1073741824, -536870913]
        } else {
          const vP = !uP ? (dmPromiseP = new PromiseExternal()) : null;
          const vQ = !uQ ? (dmPromiseQ = new PromiseExternal()) : null;
          if (uQ) await uQ; else if (uP) await uP;
          if (vP) t = await dmResolve(vP);
          if (vQ) t = await dmResolve(vQ);
        }
        return t;
      }
      const inExec = new Set();
      const wFunc = async (handler, wStore) => {
        try {
          const ct = Date.now();
          if (ct - wStore.dt < 800) {
            const cid = wStore.cid;
            inExec.add(cid);
            const t = await eFunc();
            const didNotRemove = inExec.delete(cid); // true for valid key
            if (!didNotRemove || t === wStore.lastExecution) return;
            wStore.lastExecution = t;
          }
          wStore.dt = ct;
          handler();
        } catch (e) {
          console.error(e);
          throw e;
        }
      };
      const sFunc = (propFunc) => {
        return (func, ms = 0, ...args) => {
          if (typeof func === 'function') { // ignore all non-function parameter (e.g. string)
            const wStore = { dt: Date.now() };
            return (wStore.cid = propFunc(wFunc, ms, (args.length > 0 ? func.bind(null, ...args) : func), wStore));
          } else {
            return propFunc(func, ms, ...args);
          }
        };
      };
      win.setTimeout = sFunc(setTimeout);
      win.setInterval = sFunc(setInterval);

      const dFunc = (propFunc) => {
        return (cid) => {
          if (cid) inExec.delete(cid) || propFunc(cid);
        };
      };

      win.clearTimeout = dFunc(clearTimeout);
      win.clearInterval = dFunc(clearInterval);

      try {
        win.setTimeout.toString = setTimeout.toString.bind(setTimeout);
        win.setInterval.toString = setInterval.toString.bind(setInterval);
        win.clearTimeout.toString = clearTimeout.toString.bind(clearTimeout);
        win.clearInterval.toString = clearInterval.toString.bind(clearInterval);
      } catch (e) { console.warn(e) }

    })();

    /*
        let mInterupter = null;
        setInterval(() => {
        if (mInterupter === afInterupter) {
            if (mInterupter !== null) afInterupter = mInterupter = (mInterupter(), null);
        } else {
            mInterupter = afInterupter;
        }
        }, 125);
    */
  });

});
