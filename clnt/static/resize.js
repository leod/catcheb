// https://github.com/ryanisaacg/quicksilver/issues/628#issuecomment-670566767
// https://stackoverflow.com/questions/4288253/html5-canvas-100-width-height-of-viewport/8486324#8486324
(async () => {
    for (;;) {
        await new Promise(r => setTimeout(r, 10));

        const canvas = document.querySelector("canvas");
        if (typeof canvas !== "undefined" && canvas != null) {
            function resize() {
                canvas.width = window.innerWidth;
                canvas.height = window.innerHeight;
            }

            window.addEventListener("resize", resize);

            resize();

            return;
        }
    }
})();
