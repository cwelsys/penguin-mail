(function () {
  let serial = 0;
  let current = null;
  let focused = null;
  let hovered = null;
  const preview = document.getElementById('mailrs-link-preview');
  const load = document.documentElement.getAttribute('data-load') || '';
  if (!preview) return;
  const hide = function () {
    serial += 1;
    current = null;
    preview.hidden = true;
    preview.textContent = '';
  };
  const clear = function () {
    focused = null;
    hovered = null;
    hide();
  };
  window.mailrsHideLinkPreview = clear;
  window.mailrsShowLinkPreview = function (reply) {
    if (reply.load !== load || reply.serial !== serial) return;
    if (!reply.link) { hide(); return; }
    preview.textContent = '';
    [['strong', reply.link.site], ['span', reply.link.address], ['small', reply.link.reason]].forEach(function (part) {
      if (!part[1]) return;
      const words = document.createElement(part[0]);
      words.textContent = part[1];
      words.dir = 'ltr';
      preview.appendChild(words);
    });
    preview.classList.toggle('caution', Boolean(reply.link.reason));
    preview.hidden = false;
  };
  const show = function (anchor) {
    if (!anchor) { hide(); return; }
    if (anchor === current) return;
    current = anchor;
    const href = anchor.getAttribute('href');
    window.webkit.messageHandlers.mailrsLink.postMessage(JSON.stringify({
      load: load, serial: ++serial, href: href, text: anchor.innerText || anchor.textContent || ''
    }));
  };
  const anchorOf = function (event) {
    return event.composedPath().find(function (node) {
      return node instanceof HTMLAnchorElement && node.hasAttribute('href');
    });
  };
  const watched = new WeakSet();
  const watch = function (root) {
    if (watched.has(root)) return;
    watched.add(root);
    // WebKit can deliver native hover events only inside the shadow root.
    // Listening on the document alone misses the link inside a message.
    root.addEventListener('mouseover', function (event) {
      hovered = anchorOf(event);
      show(hovered || focused);
    }, true);
    root.addEventListener('focusin', function (event) {
      focused = anchorOf(event);
      show(focused);
    }, true);
    root.addEventListener('mouseout', function (event) {
      if (!event.relatedTarget || !hovered || !hovered.contains(event.relatedTarget)) {
        hovered = null;
        show(focused);
      }
    }, true);
    root.addEventListener('focusout', function () {
      focused = null;
      show(hovered);
    }, true);
    observer.observe(root === document ? document.body : root, { childList: true, subtree: true });
    watchTree(root);
  };
  const watchTree = function (node) {
    if (node instanceof Element && node.shadowRoot) watch(node.shadowRoot);
    if (!node.querySelectorAll) return;
    node.querySelectorAll('*').forEach(function (child) {
      if (child.shadowRoot) watch(child.shadowRoot);
    });
  };
  window.addEventListener('blur', clear);
  window.addEventListener('scroll', function () {
    // Scrolling invalidates the pointer target, but keyboard focus stays
    // on its link, including when the browser scrolls that link into view.
    hovered = null;
    show(focused && focused.isConnected ? focused : null);
  }, true);
  document.addEventListener('visibilitychange', clear);
  // A patch replaces an article under a container, so the observer has to
  // see the whole tree. Filling the preview is also a change under body,
  // and that one must not take the preview down.
  const insidePreview = function (node) {
    return node === preview || preview.contains(node);
  };
  const observer = new MutationObserver(function (records) {
    let changed = false;
    for (const record of records) {
      if (insidePreview(record.target)) continue;
      changed = true;
      record.addedNodes.forEach(watchTree);
    }
    if (changed) clear();
  });
  watch(document);
})()
