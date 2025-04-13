foo::bar { 'yo': }
file { '/tmp/one':
  mode    => '0644',
  content => "bi${loo}ba",
}
file { '/tmp/two':
  ensure => 'directory',
  mode   => '0750',
}
file { '/tmp/two/three':
  ensure => 'directory',
  mode   => '0750',
}
file { '/tmp/two/four':
  ensure  => 'directory',
  mode    => '0750',
  content => "bo${jaz}bi",
}
service { 'nginx':
  ensure => 'running',
}
service { 'ssh':
  ensure => 'running',
}
exec { "/root/${scripts}/yo.sh": }

File['/tmp/two'] -> [File['/tmp/two/three'], File['/tmp/two/four']] ~> Service['nginx']

Service['ssh'] <~ File['/tmp/one'] <- Exec["/root/${scripts}/yo.sh"]

Foo::Bar['yo'] ~> Service['ssh']
