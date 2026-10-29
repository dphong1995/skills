// Until answered, closing or reloading asks first (the browser's own dialog, asked for only after the user clicked or typed).
// Chrome lets a page close its own tab only while the tab history has one entry, and every <a href="#…"> click adds one,
// so guarded pages jump with location.replace (it scrolls like the link, honours scroll-padding-top, sets :target).
let answered = false;
export const guardLeave = () => {
  addEventListener('beforeunload', (e) => { if (!answered && navigator.userActivation.hasBeenActive) e.preventDefault(); });
  document.addEventListener('click', (e) => {
    const a = e.target instanceof Element && e.target.closest('a[href^="#"]');
    if (!a || e.defaultPrevented || e.button || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    location.replace(a.getAttribute('href'));
  });
};
export const markAnswered = () => { answered = true; };
export const closeTab = () => { answered = true; window.close(); };
