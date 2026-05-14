const copyButtons = document.querySelectorAll("[data-copy-target]");
const themeToggle = document.querySelector("[data-theme-toggle]");
const themeLabel = document.querySelector("[data-theme-label]");
const themeOrder = ["system", "light", "dark"];

function getStoredTheme() {
  try {
    return localStorage.getItem("orqa-theme") || "system";
  } catch {
    return "system";
  }
}

function setTheme(theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.dataset.theme = theme;
  }

  try {
    localStorage.setItem("orqa-theme", theme);
  } catch {
    // Theme still changes for the current page view when storage is blocked.
  }

  themeLabel.textContent = theme.charAt(0).toUpperCase() + theme.slice(1);
}

async function copyText(button, targetId) {
  const target = document.getElementById(targetId);

  if (!target) {
    return;
  }

  const originalText = button.textContent;
  const text = "value" in target ? target.value : target.textContent.trim();

  if (navigator.clipboard) {
    await navigator.clipboard.writeText(text);
  } else {
    const fallback = document.createElement("textarea");
    fallback.value = text;
    fallback.setAttribute("readonly", "");
    fallback.style.position = "fixed";
    fallback.style.opacity = "0";
    document.body.appendChild(fallback);
    fallback.select();
    document.execCommand("copy");
    fallback.remove();
  }

  button.textContent = "Copied";

  window.setTimeout(() => {
    button.textContent = originalText;
  }, 1400);
}

copyButtons.forEach((button) => {
  button.addEventListener("click", () => {
    copyText(button, button.dataset.copyTarget).catch(() => {
      button.textContent = "Copy Failed";
    });
  });
});

themeToggle.addEventListener("click", () => {
  const currentTheme = getStoredTheme();
  const nextTheme = themeOrder[(themeOrder.indexOf(currentTheme) + 1) % themeOrder.length];
  setTheme(nextTheme);
});

setTheme(getStoredTheme());
