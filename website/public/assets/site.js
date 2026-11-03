// 2drivescan.app — platform detection, navigation and the live demo.
(() => {
  document.documentElement.classList.remove('no-js');

  // ------------------------------------------------------------ platform
  function detectOs() {
    const ua = navigator.userAgent || '';
    const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';
    if (/android|iphone|ipad|ipod/i.test(ua)) return 'mobile';
    if (/mac/i.test(platform) || /Mac OS X/.test(ua)) return 'macos';
    if (/win/i.test(platform) || /Windows/.test(ua)) return 'windows';
    if (/linux|x11|cros/i.test(platform) || /Linux/.test(ua)) return 'linux';
    return null;
  }

  const os = detectOs();

  document.querySelectorAll('[data-download-cta]').forEach((cta) => {
    let options = {};
    try {
      options = JSON.parse(cta.dataset.options || '{}');
    } catch {
      return;
    }
    const choice = os && options[os];
    if (!choice) return;
    const link = cta.querySelector('[data-cta-link]');
    const label = cta.querySelector('[data-cta-label]');
    const meta = cta.querySelector('[data-cta-meta]');
    if (link) link.href = choice.href;
    if (label) label.textContent = choice.label;
    if (meta) meta.textContent = choice.meta;
  });

  if (os && os !== 'mobile') document.querySelectorAll(`[data-os="${os}"]`).forEach((el) => el.classList.add('detected'));

  // ------------------------------------------------------------ navigation
  const nav = document.querySelector('[data-nav]');
  const toggle = document.querySelector('[data-nav-toggle]');
  if (nav && toggle) {
    toggle.addEventListener('click', () => {
      const open = nav.classList.toggle('open');
      toggle.setAttribute('aria-expanded', String(open));
    });
    nav.querySelectorAll('.nav-links a').forEach((a) =>
      a.addEventListener('click', () => {
        nav.classList.remove('open');
        toggle.setAttribute('aria-expanded', 'false');
      }),
    );
  }
  const onScroll = () => nav && nav.classList.toggle('scrolled', window.scrollY > 8);
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ------------------------------------------------------------ reveal on scroll
  const revealed = document.querySelectorAll('.reveal');
  if ('IntersectionObserver' in window) {
    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((e) => {
          if (e.isIntersecting) {
            e.target.classList.add('in');
            io.unobserve(e.target);
          }
        });
      },
      { rootMargin: '0px 0px -8% 0px' },
    );
    revealed.forEach((el) => io.observe(el));
  } else {
    revealed.forEach((el) => el.classList.add('in'));
  }

  // ------------------------------------------------------------ live demo
  const body = document.querySelector('[data-demo-frame]');
  if (!body) return;
  const iframe = body.querySelector('iframe');
  const start = body.querySelector('[data-demo-start]');
  const small = () => window.matchMedia('(max-width: 760px)').matches;

  const fit = () => {
    const scale = body.clientWidth / 1320;
    iframe.style.transform = `scale(${scale})`;
  };
  new ResizeObserver(fit).observe(body);
  fit();

  let loaded = false;
  const load = () => {
    if (loaded) return;
    loaded = true;
    iframe.addEventListener('load', () => body.classList.add('live'), { once: true });
    iframe.src = iframe.dataset.src;
    iframe.tabIndex = 0;
  };

  start.addEventListener('click', () => {
    // On phones the scaled app is too small to use: open it full screen.
    if (small()) window.open(iframe.dataset.src, '_blank', 'noopener');
    else load();
  });

  // On larger screens the demo starts by itself when it scrolls into view.
  if (!small() && 'IntersectionObserver' in window) {
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        io.disconnect();
        setTimeout(load, 250);
      }
    }, { threshold: 0.35 });
    io.observe(body);
  }
})();
