package tui

import (
	"fmt"
	"strings"

	"charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/user/nd/internal/compose"
	"github.com/user/nd/internal/dockerfile"
	"github.com/user/nd/internal/nerdctl"
)

// Action represents an action the user can take
type Action string

const (
	ActionStart   Action = "start"
	ActionDown    Action = "down"
	ActionEnter   Action = "enter"
	ActionLogs    Action = "logs"
	ActionRestart Action = "restart"
	ActionBuild   Action = "build"
	ActionExit    Action = "exit"
)

type actionItem struct {
	action  Action
	label   string
	key     string
	icon    string
	enabled bool
}

var actionItems = []actionItem{
	{ActionStart, "Start services", "1", "▶", true},
	{ActionDown, "Stop services", "2", "■", true},
	{ActionEnter, "Enter container", "3", "→", true},
	{ActionLogs, "View logs", "4", "☰", true},
	{ActionRestart, "Restart", "5", "↻", true},
	{ActionBuild, "Build image", "6", "⚙", true},
	{ActionExit, "Exit", "q", "✕", true},
}

// Color palette
var (
	colorPrimary   = lipgloss.Color("86")  // Cyan-green
	colorAccent    = lipgloss.Color("170") // Purple
	colorMuted     = lipgloss.Color("241") // Gray
	colorText      = lipgloss.Color("252") // Light gray
	colorSuccess   = lipgloss.Color("82")  // Green
	colorError     = lipgloss.Color("196") // Red
	colorWarning   = lipgloss.Color("214") // Orange
	colorRunning   = lipgloss.Color("82")  // Green
	colorStopped   = lipgloss.Color("241") // Gray
	colorBorder    = lipgloss.Color("62")  // Blue
	colorHighlight = lipgloss.Color("229") // Yellow
)

// Styles
var (
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorPrimary).
			Padding(0, 1)

	subtitleStyle = lipgloss.NewStyle().
			Foreground(colorMuted).
			Padding(0, 1)

	selectedStyle = lipgloss.NewStyle().
			Foreground(colorAccent).
			Bold(true)

	normalStyle = lipgloss.NewStyle().
			Foreground(colorText)

	mutedStyle = lipgloss.NewStyle().
			Foreground(colorMuted)

	successStyle = lipgloss.NewStyle().
			Foreground(colorSuccess)

	errorStyle = lipgloss.NewStyle().
			Foreground(colorError)

	boxStyle = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(colorBorder).
			Padding(1, 2)

	actionStyle = lipgloss.NewStyle().
			Padding(0, 1)

	helpStyle = lipgloss.NewStyle().
			Foreground(colorMuted).
			MarginTop(1)

	keyStyle = lipgloss.NewStyle().
			Foreground(colorHighlight).
			Bold(true)

	bannerStyle = lipgloss.NewStyle().
			Padding(0, 1).
			MarginTop(1)

	statusRunningStyle = lipgloss.NewStyle().
				Foreground(colorRunning).
				Bold(true)

	statusStoppedStyle = lipgloss.NewStyle().
				Foreground(colorStopped)

	inputCursorStyle = lipgloss.NewStyle().
				Foreground(colorAccent).
				Bold(true)
)

// state represents the current UI state
type state int

const (
	stateMenu state = iota
	stateSelectService
	stateSelectDockerfile
	stateEditArgs
	stateInputArg
)

// ServiceStatus represents the status of a service
type ServiceStatus struct {
	Name    string
	Running bool
	State   string
}

// Model is the main TUI model
type Model struct {
	state              state
	services           []ServiceStatus
	selected           int
	selectedSvc        int
	composeFile        string
	err                error
	message            string
	messageType        string // "success", "error", "info"
	executeAction      Action
	executeService     string
	dockerfiles        []string
	selectedDockerfile int
	dockerfileArgs     []dockerfile.Arg
	argValues          map[string]string
	selectedArg        int
	buildTag           string
	executeDockerfile  string
	executeArgValues   map[string]string
	executeBuildTag    string
	inputBuffer        string
	width              int
	height             int
}

// NewModel creates a new TUI model
func NewModel(services []string, composeFile string) Model {
	serviceStatuses := make([]ServiceStatus, len(services))
	for i, s := range services {
		serviceStatuses[i] = ServiceStatus{Name: s, Running: false, State: "unknown"}
	}
	return Model{
		state:       stateMenu,
		services:    serviceStatuses,
		selected:    0,
		selectedSvc: 0,
		composeFile: composeFile,
	}
}

// Init initializes the model
func (m Model) Init() tea.Cmd {
	return nil
}

// Update handles events
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case tea.KeyPressMsg:
		// Handle text input mode
		if m.state == stateInputArg {
			return m.handleInputKey(msg)
		}

		if msg.String() == "ctrl+c" {
			return m, tea.Quit
		}

		switch msg.Code {
		case tea.KeyUp:
			return m.moveUp(), nil

		case tea.KeyDown:
			return m.moveDown(), nil

		case tea.KeyEnter:
			return m.handleSelection()

		case tea.KeyTab:
			if m.state == stateEditArgs && len(m.dockerfileArgs) > 0 {
				m.selectedArg = (m.selectedArg + 1) % len(m.dockerfileArgs)
			}

		case tea.KeyEscape:
			if m.state != stateMenu {
				m.state = stateMenu
				m.err = nil
				m.message = ""
				return m, nil
			}
		}

		// Character keys
		switch msg.Text {
		case "q":
			if m.state == stateMenu {
				return m, tea.Quit
			}
			m.state = stateMenu
			m.err = nil
			m.message = ""
			return m, nil

		case "k":
			return m.moveUp(), nil

		case "j":
			return m.moveDown(), nil

		case " ":
			return m.handleSelection()

		case "e":
			if m.state == stateEditArgs && len(m.dockerfileArgs) > 0 {
				argName := m.dockerfileArgs[m.selectedArg].Name
				m.inputBuffer = m.argValues[argName]
				m.state = stateInputArg
				return m, nil
			}

		case "1", "2", "3", "4", "5", "6":
			if m.state == stateMenu {
				idx := int(msg.Text[0] - '1')
				if idx < len(actionItems) {
					m.selected = idx
					return m.handleSelection()
				}
			}
		}

	case executionCompleteMsg:
		m.state = stateMenu
		m.message = msg.message
		if msg.err != nil {
			m.err = msg.err
			m.messageType = "error"
		} else {
			m.messageType = "success"
		}
		return m, nil
	}

	return m, nil
}

func (m Model) handleInputKey(msg tea.KeyPressMsg) (tea.Model, tea.Cmd) {
	switch msg.Code {
	case tea.KeyEnter:
		if len(m.dockerfileArgs) > 0 && m.selectedArg < len(m.dockerfileArgs) {
			argName := m.dockerfileArgs[m.selectedArg].Name
			m.argValues[argName] = m.inputBuffer
		}
		m.state = stateEditArgs
		m.inputBuffer = ""
		return m, nil
	case tea.KeyEscape:
		m.state = stateEditArgs
		m.inputBuffer = ""
		return m, nil
	case tea.KeyBackspace:
		if len(m.inputBuffer) > 0 {
			m.inputBuffer = m.inputBuffer[:len(m.inputBuffer)-1]
		}
		return m, nil
	}

	// Character input
	if msg.Text != "" && msg.Code == 0 {
		m.inputBuffer += msg.Text
	}
	return m, nil
}

func (m Model) moveUp() Model {
	switch m.state {
	case stateMenu:
		if m.selected > 0 {
			m.selected--
		}
	case stateSelectService:
		if m.selectedSvc > 0 {
			m.selectedSvc--
		}
	case stateSelectDockerfile:
		if m.selectedDockerfile > 0 {
			m.selectedDockerfile--
		}
	case stateEditArgs:
		if m.selectedArg > 0 {
			m.selectedArg--
		}
	}
	return m
}

func (m Model) moveDown() Model {
	switch m.state {
	case stateMenu:
		if m.selected < len(actionItems)-1 {
			m.selected++
		}
	case stateSelectService:
		if m.selectedSvc < len(m.services)-1 {
			m.selectedSvc++
		}
	case stateSelectDockerfile:
		if m.selectedDockerfile < len(m.dockerfiles)-1 {
			m.selectedDockerfile++
		}
	case stateEditArgs:
		if m.selectedArg < len(m.dockerfileArgs)-1 {
			m.selectedArg++
		}
	}
	return m
}

// handleSelection handles the user's selection
func (m Model) handleSelection() (tea.Model, tea.Cmd) {
	if m.state == stateEditArgs {
		m.executeAction = ActionBuild
		m.executeDockerfile = m.dockerfiles[m.selectedDockerfile]
		m.executeArgValues = make(map[string]string)
		for _, arg := range m.dockerfileArgs {
			m.executeArgValues[arg.Name] = m.argValues[arg.Name]
		}
		m.executeBuildTag = m.buildTag
		return m, tea.Quit
	}

	if m.state == stateSelectDockerfile {
		parser := dockerfile.NewParser()
		df, err := parser.Parse(m.dockerfiles[m.selectedDockerfile])
		if err != nil {
			m.err = err
			m.messageType = "error"
			return m, nil
		}

		m.dockerfileArgs = df.Args
		m.argValues = make(map[string]string)
		for _, arg := range df.Args {
			if arg.HasDefault {
				m.argValues[arg.Name] = arg.DefaultValue
			} else {
				m.argValues[arg.Name] = ""
			}
		}
		m.selectedArg = 0
		m.state = stateEditArgs
		return m, nil
	}

	if m.state == stateSelectService {
		m.executeAction = actionItems[m.selected].action
		m.executeService = m.services[m.selectedSvc].Name
		return m, tea.Quit
	}

	item := actionItems[m.selected]
	switch item.action {
	case ActionExit:
		return m, tea.Quit

	case ActionStart, ActionDown:
		m.executeAction = item.action
		return m, tea.Quit

	case ActionEnter, ActionLogs, ActionRestart:
		if len(m.services) == 1 {
			m.executeAction = item.action
			m.executeService = m.services[0].Name
			return m, tea.Quit
		}
		m.state = stateSelectService
		m.selectedSvc = 0
		return m, nil

	case ActionBuild:
		var err error
		m, err = m.startBuildSelection()
		if err != nil {
			m.err = err
			m.messageType = "error"
		}
		return m, nil
	}

	return m, nil
}

func (m Model) startBuildSelection() (Model, error) {
	parser := dockerfile.NewParser()
	dockerfiles, err := parser.FindDockerfiles()
	if err != nil {
		return m, err
	}
	if len(dockerfiles) == 0 {
		return m, fmt.Errorf("no Dockerfiles found in current directory")
	}
	m.dockerfiles = dockerfiles
	m.selectedDockerfile = 0
	m.state = stateSelectDockerfile
	return m, nil
}

// executionCompleteMsg is sent when a command finishes
type executionCompleteMsg struct {
	message string
	err     error
}

// executeCommand executes a nerdctl command
func executeCommand(action Action, service string) tea.Cmd {
	return func() tea.Msg {
		runner := nerdctl.NewRunner()
		var cmd nerdctl.Command

		switch action {
		case ActionStart:
			cmd = nerdctl.CommandUp
		case ActionDown:
			cmd = nerdctl.CommandDown
		case ActionEnter:
			cmd = nerdctl.CommandEnter
		case ActionLogs:
			cmd = nerdctl.CommandLogs
		case ActionRestart:
			cmd = nerdctl.CommandRestart
		case ActionExit:
			return executionCompleteMsg{message: "Exited"}
		}

		err := runner.Run(cmd, service)
		return executionCompleteMsg{
			message: fmt.Sprintf("Command '%s' completed", action),
			err:     err,
		}
	}
}

// View renders the TUI
func (m Model) View() tea.View {
	var b strings.Builder

	// Title
	b.WriteString(titleStyle.Render("nd"))
	b.WriteString(subtitleStyle.Render(" nerdctl compose manager"))
	b.WriteString("\n\n")

	// Compose file info
	b.WriteString(mutedStyle.Render("compose: "))
	b.WriteString(normalStyle.Render(m.composeFile))
	b.WriteString("\n\n")

	// Handle different states
	switch m.state {
	case stateEditArgs, stateInputArg:
		m.renderBuildView(&b)
	case stateSelectDockerfile:
		m.renderDockerfileSelect(&b)
	case stateSelectService:
		m.renderServiceSelect(&b)
	default:
		m.renderMainMenu(&b)
	}

	// Message/Error banner
	if m.message != "" || m.err != nil {
		b.WriteString("\n")
		if m.err != nil {
			b.WriteString(bannerStyle.Foreground(colorError).Render("✗ " + m.err.Error()))
		} else if m.message != "" {
			b.WriteString(bannerStyle.Foreground(colorSuccess).Render("✓ " + m.message))
		}
	}

	v := tea.NewView(b.String())
	v.AltScreen = true
	v.MouseMode = tea.MouseModeCellMotion
	return v
}

func (m Model) renderMainMenu(b *strings.Builder) {
	// Actions section
	b.WriteString(normalStyle.Render("Actions:"))
	b.WriteString("\n\n")

	for i, item := range actionItems {
		prefix := "  "
		if i == m.selected {
			prefix = selectedStyle.Render("→ ")
			b.WriteString(prefix)
			b.WriteString(selectedStyle.Render(fmt.Sprintf("%s  %s", item.icon, item.label)))
		} else {
			b.WriteString(prefix)
			b.WriteString(normalStyle.Render(fmt.Sprintf("%s  %s", item.icon, item.label)))
		}
		b.WriteString(mutedStyle.Render(fmt.Sprintf("  [%s]", item.key)))
		b.WriteString("\n")
	}

	// Services section
	if len(m.services) > 0 {
		b.WriteString("\n")
		b.WriteString(normalStyle.Render("Services:"))
		b.WriteString("\n")
		for _, svc := range m.services {
			// statusIcon := "○"
			// statusText := "stopped"
			// if svc.Running {
			// 	statusIcon = "●"
			// 	statusText = "running"
			// }
			// b.WriteString(fmt.Sprintf("  %s %s ", statusIcon, svc.Name))
			// b.WriteString(mutedStyle.Render(statusText))
			fmt.Fprintf(b, " %s", svc.Name)
			b.WriteString("\n")
		}
	}

	// Help
	b.WriteString(helpStyle.Render("\n↑/k up  ↓/j down  1-6 quick select  q quit"))
}

func (m Model) renderServiceSelect(b *strings.Builder) {
	b.WriteString(selectedStyle.Render("Select a service:"))
	b.WriteString("\n\n")

	for i, svc := range m.services {
		prefix := "  "
		if i == m.selectedSvc {
			prefix = selectedStyle.Render("→ ")
			b.WriteString(prefix)
			b.WriteString(selectedStyle.Render(svc.Name))
		} else {
			b.WriteString(prefix)
			b.WriteString(normalStyle.Render(svc.Name))
		}

		// Show status
		if svc.Running {
			b.WriteString(statusRunningStyle.Render(" ● running"))
		} else {
			b.WriteString(statusStoppedStyle.Render(" ○ stopped"))
		}
		b.WriteString("\n")
	}

	b.WriteString(helpStyle.Render("\n↑/k up  ↓/j down  enter select  q back"))
}

func (m Model) renderDockerfileSelect(b *strings.Builder) {
	b.WriteString(selectedStyle.Render("Select a Dockerfile:"))
	b.WriteString("\n\n")

	for i, df := range m.dockerfiles {
		prefix := "  "
		if i == m.selectedDockerfile {
			prefix = selectedStyle.Render("→ ")
			b.WriteString(prefix)
			b.WriteString(selectedStyle.Render(df))
		} else {
			b.WriteString(prefix)
			b.WriteString(normalStyle.Render(df))
		}
		b.WriteString("\n")
	}

	b.WriteString(helpStyle.Render("\n↑/k up  ↓/j down  enter select  q back"))
}

func (m Model) renderBuildView(b *strings.Builder) {
	b.WriteString(selectedStyle.Render("Build: "))
	b.WriteString(normalStyle.Render(m.dockerfiles[m.selectedDockerfile]))
	b.WriteString("\n\n")

	if len(m.dockerfileArgs) == 0 {
		b.WriteString(mutedStyle.Render("No ARGs in Dockerfile. Press enter to build."))
	} else {
		b.WriteString(normalStyle.Render("Build arguments:"))
		b.WriteString("\n\n")

		for i, arg := range m.dockerfileArgs {
			value := m.argValues[arg.Name]
			prefix := "  "

			if i == m.selectedArg {
				prefix = selectedStyle.Render("→ ")
				b.WriteString(prefix)

				if m.state == stateInputArg {
					b.WriteString(selectedStyle.Render(fmt.Sprintf("%s = ", arg.Name)))
					b.WriteString(inputCursorStyle.Render(m.inputBuffer + "│"))
				} else {
					b.WriteString(selectedStyle.Render(fmt.Sprintf("%s = %s", arg.Name, value)))
				}
			} else {
				b.WriteString(prefix)
				b.WriteString(normalStyle.Render(fmt.Sprintf("%s = %s", arg.Name, value)))
			}

			if !arg.HasDefault {
				b.WriteString(mutedStyle.Render(" (required)"))
			}
			b.WriteString("\n")
		}
	}

	if m.state == stateInputArg {
		b.WriteString(helpStyle.Render("\nenter confirm  esc cancel"))
	} else {
		b.WriteString(helpStyle.Render("\n↑/k up  ↓/j down  e edit  enter build  q back"))
	}
}

// Run starts the TUI and returns any selected action to execute after exit.
func Run(services []string, composeFile string) (Action, string, error) {
	p := tea.NewProgram(
		NewModel(services, composeFile),
	)

	model, err := p.Run()
	if err != nil {
		return ActionExit, "", err
	}

	m, ok := model.(Model)
	if !ok {
		return ActionExit, "", nil
	}

	return m.executeAction, m.executeService, nil
}

// BuildResult contains the result of a build action from TUI
type BuildResult struct {
	Dockerfile string
	Args       map[string]string
	Tag        string
}

// RunForBuild starts the TUI and returns build-related results
func RunForBuild(services []string, composeFile string) (Action, string, BuildResult, error) {
	p := tea.NewProgram(
		NewModel(services, composeFile),
	)

	model, err := p.Run()
	if err != nil {
		return ActionExit, "", BuildResult{}, err
	}

	m, ok := model.(Model)
	if !ok {
		return ActionExit, "", BuildResult{}, nil
	}

	buildResult := BuildResult{
		Dockerfile: m.executeDockerfile,
		Args:       m.executeArgValues,
		Tag:        m.executeBuildTag,
	}

	return m.executeAction, m.executeService, buildResult, nil
}

// RunForBuildOnly starts the TUI directly in build selection mode.
func RunForBuildOnly() (BuildResult, error) {
	m, err := NewModel(nil, "").startBuildSelection()
	if err != nil {
		return BuildResult{}, err
	}

	p := tea.NewProgram(m)
	model, err := p.Run()
	if err != nil {
		return BuildResult{}, err
	}

	m, ok := model.(Model)
	if !ok {
		return BuildResult{}, nil
	}

	return BuildResult{
		Dockerfile: m.executeDockerfile,
		Args:       m.executeArgValues,
		Tag:        m.executeBuildTag,
	}, nil
}

// RunOnce executes a command without TUI (for direct execution)
func RunOnce(action Action, service string) error {
	runner := nerdctl.NewRunner()

	var cmd nerdctl.Command
	switch action {
	case ActionStart:
		cmd = nerdctl.CommandUp
	case ActionDown:
		cmd = nerdctl.CommandDown
	case ActionEnter:
		cmd = nerdctl.CommandEnter
	case ActionLogs:
		cmd = nerdctl.CommandLogs
	case ActionRestart:
		cmd = nerdctl.CommandRestart
	}

	return runner.Run(cmd, service)
}

// RunBuild executes a build command
func RunBuild(dockerfile string, args map[string]string, tag string) error {
	runner := nerdctl.NewRunner()
	return runner.Build(dockerfile, args, tag)
}

// GetActionFromString converts a string to an Action
func GetActionFromString(s string) Action {
	switch strings.ToLower(s) {
	case "start", "up":
		return ActionStart
	case "down", "stop":
		return ActionDown
	case "enter", "exec", "shell":
		return ActionEnter
	case "logs":
		return ActionLogs
	case "restart":
		return ActionRestart
	case "build":
		return ActionBuild
	default:
		return ActionExit
	}
}

// GetServices returns the services from a compose file
func GetServices(parser *compose.Parser, filename string) ([]string, error) {
	c, err := parser.Parse(filename)
	if err != nil {
		return nil, err
	}
	return parser.GetServiceNames(c), nil
}
