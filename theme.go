package main

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/theme"
)

type pastelTheme struct{}

func (pastelTheme) Color(name fyne.ThemeColorName, _ fyne.ThemeVariant) color.Color {
	switch name {
	case theme.ColorNameBackground:
		return color.NRGBA{R: 29, G: 29, B: 38, A: 255}
	case theme.ColorNameHeaderBackground:
		return color.NRGBA{R: 42, G: 41, B: 54, A: 255}
	case theme.ColorNameInputBackground:
		return color.NRGBA{R: 53, G: 51, B: 67, A: 255}
	case theme.ColorNameButton, theme.ColorNameDisabledButton, theme.ColorNameMenuBackground:
		return color.NRGBA{R: 63, G: 60, B: 77, A: 255}
	case theme.ColorNameOverlayBackground:
		return color.NRGBA{R: 39, G: 38, B: 51, A: 255}
	case theme.ColorNameForeground:
		return color.NRGBA{R: 241, G: 237, B: 246, A: 255}
	case theme.ColorNameDisabled, theme.ColorNamePlaceHolder:
		return color.NRGBA{R: 173, G: 166, B: 188, A: 255}
	case theme.ColorNamePrimary:
		return color.NRGBA{R: 186, G: 171, B: 218, A: 255}
	case theme.ColorNameForegroundOnPrimary:
		return color.NRGBA{R: 29, G: 25, B: 38, A: 255}
	case theme.ColorNameInputBorder, theme.ColorNameSeparator:
		return color.NRGBA{R: 83, G: 77, B: 98, A: 255}
	case theme.ColorNameFocus, theme.ColorNameSelection:
		return color.NRGBA{R: 186, G: 171, B: 218, A: 70}
	}
	return theme.DefaultTheme().Color(name, theme.VariantDark)
}

func (pastelTheme) Font(style fyne.TextStyle) fyne.Resource {
	return theme.DefaultTheme().Font(style)
}

func (pastelTheme) Icon(name fyne.ThemeIconName) fyne.Resource {
	return theme.DefaultTheme().Icon(name)
}

func (pastelTheme) Size(name fyne.ThemeSizeName) float32 {
	return theme.DefaultTheme().Size(name)
}
