package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/user/nd/internal/compose"
	"github.com/user/nd/internal/nerdctl"
)

// Action represents an action the user can take
type Action string

const (
	ActionStart   Action = "Start (up -d)"
	ActionDown    Action = "Down"
	ActionEnter   Action = "Enter (exec -it)"
	ActionLogs    Action = "Logs (follow)"
	ActionRestart Action = "Restart"
	ActionExit    Action = "Exit"
)

var actions = []Action{
	ActionStart,
	ActionDown,
	ActionEnter,
	ActionLogs,
	ActionRestart,
	ActionExit,
}

// styles for the TUI
var (
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("86")).
			MarginBottom(1)

	selectedStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("170")).
			Bold(true)

	normalStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("252"))

	boxStyle = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("62")).
			Padding(0, 1)

	helpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			MarginTop(1)
)

// state represents the current UI state
type state int

const (
	stateMenu state = iota
	stateSelectService
)

// Model is the main TUI model
type Model struct {
	state          state
	actions        []Action
	services       []string
	selected       int
	selectedSvc    int
	composeFile    string
	err            error
	message        string
	executeAction  Action
	executeService string
}

// NewModel creates a new TUI model
func NewModel(services []string, composeFile string) Model {
	return Model{
		state:       stateMenu,
		actions:     actions,
		services:    services,
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
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c", "q":
			if m.state == stateMenu {
				return m, tea.Quit
			}
			m.state = stateMenu
			return m, nil

		case "up", "k":
			if m.state == stateMenu {
				if m.selected > 0 {
					m.selected--
				}
			} else if m.state == stateSelectService {
				if m.selectedSvc > 0 {
					m.selectedSvc--
				}
			}

		case "down", "j":
			if m.state == stateMenu {
				if m.selected < len(m.actions)-1 {
					m.selected++
				}
			} else if m.state == stateSelectService {
				if m.selectedSvc < len(m.services)-1 {
					m.selectedSvc++
				}
			}

		case "enter", " ":
			return m.handleSelection()
		}

	case executionCompleteMsg:
		m.state = stateMenu
		m.message = msg.message
		if msg.err != nil {
			m.err = msg.err
		}
		return m, nil
	}

	return m, nil
}

// handleSelection handles the user's selection
func (m Model) handleSelection() (tea.Model, tea.Cmd) {
	if m.state == stateSelectService {
		m.executeAction = m.actions[m.selected]
		m.executeService = m.services[m.selectedSvc]
		return m, tea.Quit
	}

	action := m.actions[m.selected]

	switch action {
	case ActionExit:
		return m, tea.Quit

	case ActionStart, ActionDown:
		m.executeAction = action
		return m, tea.Quit

	case ActionEnter, ActionLogs, ActionRestart:
		if len(m.services) == 1 {
			m.executeAction = action
			m.executeService = m.services[0]
			return m, tea.Quit
		}
		m.state = stateSelectService
		m.selectedSvc = 0
		return m, nil
	}

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
func (m Model) View() string {
	var b strings.Builder

	b.WriteString(titleStyle.Render(" nd - nerdctl compose manager "))
	b.WriteString("\n")
	b.WriteString(fmt.Sprintf("Compose file: %s\n\n", m.composeFile))

	if m.state == stateSelectService {
		b.WriteString("Select a service:\n\n")
		for i, svc := range m.services {
			if i == m.selectedSvc {
				b.WriteString(selectedStyle.Render(fmt.Sprintf("  > %s", svc)))
			} else {
				b.WriteString(normalStyle.Render(fmt.Sprintf("    %s", svc)))
			}
			b.WriteString("\n")
		}
		b.WriteString("\n")
		b.WriteString(helpStyle.Render("↑/k: up  ↓/j: down  enter: select  q: back"))
		return boxStyle.Render(b.String())
	}

	// Main menu
	b.WriteString("Actions:\n\n")
	for i, action := range m.actions {
		if i == m.selected {
			b.WriteString(selectedStyle.Render(fmt.Sprintf("  > %s", action)))
		} else {
			b.WriteString(normalStyle.Render(fmt.Sprintf("    %s", action)))
		}
		b.WriteString("\n")
	}

	if len(m.services) > 0 {
		b.WriteString("\nServices: ")
		b.WriteString(strings.Join(m.services, ", "))
	}

	if m.message != "" {
		b.WriteString("\n\n")
		b.WriteString(normalStyle.Render(m.message))
	}

	if m.err != nil {
		b.WriteString("\n\n")
		b.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Render(fmt.Sprintf("Error: %v", m.err)))
	}

	b.WriteString(helpStyle.Render("\n\n↑/k: up  ↓/j: down  enter: select  q: quit"))

	return boxStyle.Render(b.String())
}

// Run starts the TUI and returns any selected action to execute after exit.
func Run(services []string, composeFile string) (Action, string, error) {
	p := tea.NewProgram(
		NewModel(services, composeFile),
		tea.WithAltScreen(),
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
