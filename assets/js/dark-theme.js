// Dark Theme Toggle Functionality

var ThemeToggle = {
    init: function () {
        console.log('ThemeToggle initializing...');

        // Create theme toggle button
        this.createToggleButton();

        // Load saved theme or detect system preference
        this.loadTheme();

        // Add event listener for system theme changes
        if (window.matchMedia) {
            window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function (e) {
                if (!localStorage.getItem('theme')) {
                    ThemeToggle.setTheme(e.matches ? 'dark' : 'light');
                }
            });
        }

        console.log('ThemeToggle initialized successfully');
    },

    createToggleButton: function () {
        // Find the navbar nav element
        var navbarNav = document.querySelector('.navbar-nav');
        if (!navbarNav) {
            console.log('Navbar not found, retrying in 500ms...');
            setTimeout(function () {
                ThemeToggle.createToggleButton();
            }, 500);
            return;
        }

        // Check if toggle already exists
        if (document.querySelector('.theme-toggle')) {
            console.log('Theme toggle already exists');
            return;
        }

        // Create toggle as a simple link that looks like other nav items
        var toggleLi = document.createElement('li');
        toggleLi.className = 'nav-item';

        var toggleLink = document.createElement('a');
        toggleLink.className = 'nav-link theme-toggle';
        toggleLink.setAttribute('href', '#');
        toggleLink.setAttribute('aria-label', 'Toggle dark theme');
        toggleLink.innerHTML = '<i class="fas fa-moon"></i>';

        toggleLink.addEventListener('click', function (e) {
            e.preventDefault();
            ThemeToggle.toggleTheme();
        });

        toggleLi.appendChild(toggleLink);
        navbarNav.appendChild(toggleLi);

        console.log('Theme toggle button created successfully');
    },

    getSystemTheme: function () {
        if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
            return 'dark';
        }
        return 'light';
    },

    loadTheme: function () {
        // Check if theme was already applied by inline script
        var currentTheme = document.documentElement.getAttribute('data-theme');
        var savedTheme = localStorage.getItem('theme');
        var theme = savedTheme || this.getSystemTheme();

        // If theme is already applied, just update the toggle button
        if (currentTheme) {
            this.updateToggleButton(currentTheme);
        } else {
            // Apply theme if not already set
            this.setTheme(theme);
        }
    },

    updateToggleButton: function (theme) {
        var toggleButton = document.querySelector('.theme-toggle');
        if (toggleButton) {
            if (theme === 'dark') {
                toggleButton.innerHTML = '<i class="fas fa-sun"></i>';
                toggleButton.setAttribute('aria-label', 'Toggle light theme');
            } else {
                toggleButton.innerHTML = '<i class="fas fa-moon"></i>';
                toggleButton.setAttribute('aria-label', 'Toggle dark theme');
            }
        }
    },

    setTheme: function (theme) {
        var html = document.documentElement;

        if (theme === 'dark') {
            html.setAttribute('data-theme', 'dark');
        } else {
            html.removeAttribute('data-theme');
        }

        // Update toggle button appearance
        this.updateToggleButton(theme);

        // Save theme preference
        localStorage.setItem('theme', theme);

        // Trigger navbar color recalculation
        if (typeof BeautifulJekyllJS !== 'undefined' && BeautifulJekyllJS.initNavbar) {
            setTimeout(BeautifulJekyllJS.initNavbar, 10);
        }
    },

    toggleTheme: function () {
        var currentTheme = document.documentElement.getAttribute('data-theme');
        var newTheme = currentTheme === 'dark' ? 'light' : 'dark';
        this.setTheme(newTheme);
    }
};

// Initialize when DOM is loaded
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () {
        ThemeToggle.init();
    });
} else {
    ThemeToggle.init();
}