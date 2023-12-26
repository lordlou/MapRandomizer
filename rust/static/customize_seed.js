function changeSamusSprite() {
    var enabled = document.getElementById("customSamusSpriteYes").checked;
    var spriteSelectDiv = document.getElementById("spriteSelectDiv");
    if (enabled) {
        spriteSelectDiv.classList.remove("d-none");
    } else {
        spriteSelectDiv.classList.add("d-none");
    }

    var sprites = document.getElementsByClassName("sprite");
    var selectedSpriteName = document.getElementById("samusSprite").value;
    var selectedSprite = document.getElementById("spriteButton-" + selectedSpriteName);

    // Unhighlight other sprites
    for (var i = 0; i < sprites.length; i++) {
        if (sprites[i] != selectedSprite) {
            sprites[i].classList.remove("selected");
        }
    }

    // Set the sprite selected class (to highlight it):
    if (selectedSprite !== null) {
        selectedSprite.classList.add("selected");
    }

    var selectedSpriteDisplayName = selectedSprite.getAttribute("data-display-name");
    document.getElementById("selectedSpriteDisplayName").innerHTML = selectedSpriteDisplayName;

    var selectedSpriteImage = document.getElementById("selectedSpriteImage");
    selectedSpriteImage.src = "/static/samus_sprites/" + selectedSpriteName + ".png";

    saveForm(document.getElementById("customization-form"));
}
function selectSprite(el) {
    // Set the form field:
    document.getElementById("samusSprite").value = el.getAttribute("data-name");
    changeSamusSprite();
};
function updateEnergyTankColor() {
    var enabled = document.getElementById("customEnergyTankYes").checked;
    var energyTankSelectDiv = document.getElementById("energyTankSelectDiv");
    if (enabled) {
        energyTankSelectDiv.classList.remove("d-none");
    } else {
        energyTankSelectDiv.classList.add("d-none");
    }

    var selectedETankSVG = document.getElementById("selectedETankSVG");
    var selectedETankColor = document.getElementById("etankColor").value;
    var svg = "";
    for (let y = 0; y < 2; y++) {
        for (let x = 0; x < 7; x++) {
            var xPixel = x * 24 + 3;
            var yPixel = y * 24 + 4;
            svg += `<rect x="${xPixel}" y="${yPixel}" width="18" height="15" fill="white"/>`;
            svg += `<rect x="${xPixel + 3}" y="${yPixel + 3}" width="15" height="12" fill="#${selectedETankColor}"/>`;
        }
    }
    selectedETankSVG.innerHTML = svg;

    saveForm(document.getElementById("customization-form"));
}
function changeEnergyTankColor(btn) {
    color = btn.getAttribute("data-color");
    document.getElementById("etankColor").value = color;
    updateEnergyTankColor();
}
function swapButtonAssignment(clickedEl) {
    var action = null;
    var actionIdx = null;
    var actions = ["control_shot", "control_jump", "control_dash", "control_item_select", "control_item_cancel", "control_angle_up", "control_angle_down"];
    var formEl = document.getElementById(clickedEl.getAttribute("for"));
    var newButton = formEl.value;
    var oldButton = formEl.form.elements[formEl.name].value;
    for (actionIdx in actions) {
        action = actions[actionIdx];
        if (action == formEl.name) {
            continue;
        }
        if (formEl.form.elements[action].value == newButton) {
            formEl.form.elements[action].value = oldButton;
        }
    }
}
