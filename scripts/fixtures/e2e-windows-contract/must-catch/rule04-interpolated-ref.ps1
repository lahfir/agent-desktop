function Invoke-Leg {
    $envelope = Invoke-AgentDesktop -Arguments @('click', '@s8f3k2p9:e1', '--headed')
    return $envelope
}
